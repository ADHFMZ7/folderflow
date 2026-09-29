//! Updates: Vela checks for a newer version at launch and every few hours,
//! downloads it in the background, then asks to restart. An update nobody
//! restarts for installs when Vela quits. See docs/engine.md, "Updates".

use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};

use chrono::{DateTime, Duration, Utc};
use serde::Serialize;
use tauri::{AppHandle, Manager};
use tauri_plugin_updater::UpdaterExt;
use ts_rs::TS;

use crate::api::commands::AppBackend;

/// How long after one automatic check the next is due.
pub const CHECK_EVERY: Duration = Duration::hours(6);

/// The first look, once Vela has settled after launch.
const FIRST_LOOK: std::time::Duration = std::time::Duration::from_secs(20);

/// How long quitting waits for a downloaded update to go in place.
const INSTALL_WAIT: std::time::Duration = std::time::Duration::from_secs(10);

/// How often the schedule looks whether a check is due.
const LOOK_EVERY: std::time::Duration = std::time::Duration::from_secs(30 * 60);

/// Where updates stand, for Settings, the banner and the menu bar.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(tag = "state", rename_all = "camelCase")]
#[ts(export)]
pub enum UpdateStatus {
    /// A development build, which can't replace itself.
    Unavailable,
    /// Not checking. `checkedAt` is the last check that found nothing newer.
    #[serde(rename_all = "camelCase")]
    Idle {
        checked_at: Option<String>,
    },
    Checking,
    Downloading {
        version: String,
    },
    /// Downloaded and checked: installs on restart, or when Vela quits.
    Ready {
        version: String,
        notes: String,
    },
    Installing {
        version: String,
    },
    Failed {
        message: String,
    },
}

/// A newer version, downloaded and its signature checked.
pub struct Downloaded {
    pub version: String,
    pub notes: String,
    /// Puts it in place of Vela.app. Takes effect when Vela next starts.
    pub install: Box<dyn Fn() -> Result<(), String> + Send + Sync>,
}

pub type Fetching<'a> =
    Pin<Box<dyn Future<Output = Result<Option<Downloaded>, String>> + Send + 'a>>;

/// Where new versions come from: the updater plugin, or a fake in tests.
pub trait Source: Send + Sync {
    /// The newer version on offer, downloaded, or `None` when this is the
    /// newest. `found` is told the version before the download starts.
    fn fetch<'a>(&'a self, found: Box<dyn FnOnce(&str) + Send + 'a>) -> Fetching<'a>;
}

#[derive(Clone)]
pub struct Updates {
    inner: Arc<Inner>,
}

struct Inner {
    source: Option<Box<dyn Source>>,
    state: Mutex<State>,
    changed: Box<dyn Fn(&UpdateStatus) + Send + Sync>,
}

struct State {
    status: UpdateStatus,
    downloaded: Option<Downloaded>,
    checked_at: Option<DateTime<Utc>>,
}

impl Updates {
    pub fn new(
        source: impl Source + 'static,
        changed: impl Fn(&UpdateStatus) + Send + Sync + 'static,
    ) -> Updates {
        Updates::with(
            Some(Box::new(source)),
            UpdateStatus::Idle { checked_at: None },
            changed,
        )
    }

    /// For a development build: never checks.
    pub fn unavailable() -> Updates {
        Updates::with(None, UpdateStatus::Unavailable, |_| {})
    }

    fn with(
        source: Option<Box<dyn Source>>,
        status: UpdateStatus,
        changed: impl Fn(&UpdateStatus) + Send + Sync + 'static,
    ) -> Updates {
        Updates {
            inner: Arc::new(Inner {
                source,
                state: Mutex::new(State {
                    status,
                    downloaded: None,
                    checked_at: None,
                }),
                changed: Box::new(changed),
            }),
        }
    }

    pub fn status(&self) -> UpdateStatus {
        self.lock().status.clone()
    }

    /// The version waiting to be installed, if one is.
    pub fn ready(&self) -> Option<String> {
        match &self.lock().status {
            UpdateStatus::Ready { version, .. } => Some(version.clone()),
            _ => None,
        }
    }

    /// Whether an automatic check is due at `now`.
    pub fn due(&self, now: DateTime<Utc>) -> bool {
        let state = self.lock();
        let idle = matches!(
            state.status,
            UpdateStatus::Idle { .. } | UpdateStatus::Failed { .. }
        );
        idle && state.checked_at.is_none_or(|at| now - at >= CHECK_EVERY)
    }

    /// Looks for a newer version and downloads it. Does nothing while a
    /// check is going or once one is downloaded.
    pub async fn check(&self, now: DateTime<Utc>) -> UpdateStatus {
        let Some(source) = &self.inner.source else {
            return UpdateStatus::Unavailable;
        };
        {
            let mut state = self.lock();
            if !matches!(
                state.status,
                UpdateStatus::Idle { .. } | UpdateStatus::Failed { .. }
            ) {
                return state.status.clone();
            }
            state.checked_at = Some(now);
        }
        self.set(UpdateStatus::Checking);
        let found = source
            .fetch(Box::new(|version| {
                self.set(UpdateStatus::Downloading {
                    version: version.to_owned(),
                });
            }))
            .await;
        match found {
            Ok(None) => self.set(UpdateStatus::Idle {
                checked_at: Some(now.to_rfc3339()),
            }),
            Ok(Some(downloaded)) => {
                let status = UpdateStatus::Ready {
                    version: downloaded.version.clone(),
                    notes: downloaded.notes.clone(),
                };
                self.lock().downloaded = Some(downloaded);
                self.set(status)
            }
            Err(message) => self.set(UpdateStatus::Failed { message }),
        }
    }

    /// Puts the downloaded version in place of Vela.app, for when Vela next
    /// starts. `Ok(false)` when there was nothing to install. A failed
    /// install leaves it ready, to try again.
    pub fn install(&self) -> Result<bool, String> {
        let (version, notes) = match self.status() {
            UpdateStatus::Ready { version, notes } => (version, notes),
            _ => return Ok(false),
        };
        self.set(UpdateStatus::Installing {
            version: version.clone(),
        });
        let result = match &self.lock().downloaded {
            Some(downloaded) => (downloaded.install)(),
            None => Err("The update is no longer there.".into()),
        };
        match result {
            Ok(()) => Ok(true),
            Err(e) => {
                self.set(UpdateStatus::Ready { version, notes });
                Err(e)
            }
        }
    }

    /// Installs as Vela quits. An update that fails to install is dropped
    /// rather than left to hold up the quit: the next start checks again.
    pub fn install_on_quit(&self) -> Result<bool, String> {
        let result = self.install();
        if let Err(message) = &result {
            self.lock().downloaded = None;
            self.set(UpdateStatus::Failed {
                message: message.clone(),
            });
        }
        result
    }

    /// Installs a downloaded update on Vela's way out, waiting at most
    /// `INSTALL_WAIT`. Quitting can't ask for an administrator's password,
    /// so where Vela.app's folder isn't writable the update is left for
    /// Restart to update, which can.
    pub fn install_as_quitting(&self) {
        if self.ready().is_none() || !can_replace_app() {
            return;
        }
        let updates = self.clone();
        let (done, installed) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let _ = done.send(updates.install_on_quit());
        });
        match installed.recv_timeout(INSTALL_WAIT) {
            Ok(Ok(_)) => {}
            Ok(Err(e)) => eprintln!("vela: {e}"),
            Err(_) => eprintln!("vela: the update took too long to install, so it was left"),
        }
    }

    fn set(&self, status: UpdateStatus) -> UpdateStatus {
        self.lock().status = status.clone();
        (self.inner.changed)(&status);
        status
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, State> {
        self.inner.state.lock().unwrap_or_else(|e| e.into_inner())
    }
}

/// The updater plugin, checking the endpoint in tauri.conf.json. Only
/// Vela.app uses it: the plugin replaces the app bundle it runs from.
pub struct Plugin(pub AppHandle);

impl Source for Plugin {
    fn fetch<'a>(&'a self, found: Box<dyn FnOnce(&str) + Send + 'a>) -> Fetching<'a> {
        Box::pin(async move {
            let checked = match self.0.updater() {
                Ok(updater) => updater.check().await,
                Err(e) => Err(e),
            };
            let update = checked.map_err(|e| {
                eprintln!("vela: couldn't check for updates: {e}");
                "Vela couldn't check for updates. It'll try again later.".to_string()
            })?;
            let Some(update) = update else {
                return Ok(None);
            };
            found(&update.version);
            // Checked against the public key in tauri.conf.json as it downloads.
            let bytes = update.download(|_, _| {}, || {}).await.map_err(|e| {
                eprintln!("vela: couldn't download {}: {e}", update.version);
                format!(
                    "Vela {} couldn't be downloaded. It'll try again later.",
                    update.version
                )
            })?;
            Ok(Some(Downloaded {
                version: update.version.clone(),
                notes: update.body.clone().unwrap_or_default(),
                install: Box::new(move || {
                    update.install(&bytes).map_err(|e| {
                        eprintln!("vela: couldn't install {}: {e}", update.version);
                        format!("Vela {} couldn't be installed.", update.version)
                    })
                }),
            }))
        })
    }
}

/// Whether the folder Vela.app is in can take a new Vela.app without a
/// password.
fn can_replace_app() -> bool {
    let Ok(exe) = std::env::current_exe() else {
        return false;
    };
    // …/Vela.app/Contents/MacOS/vela
    let Some(folder) = exe.ancestors().nth(4) else {
        return false;
    };
    let Ok(path) = std::ffi::CString::new(folder.as_os_str().as_encoded_bytes()) else {
        return false;
    };
    unsafe { libc::access(path.as_ptr(), libc::W_OK) == 0 }
}

/// Checks when one is due, if the setting is on: soon after launch, then
/// every six hours. Looks every half hour, by the clock on the wall, so
/// time the Mac spends asleep counts.
pub fn schedule(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(FIRST_LOOK).await;
        loop {
            let on = app
                .state::<AppBackend>()
                .get_settings()
                .map_or(true, |loaded| loaded.settings.check_for_updates);
            let updates = app.state::<Updates>().inner().clone();
            if on && updates.due(Utc::now()) {
                updates.check(Utc::now()).await;
            }
            tokio::time::sleep(LOOK_EVERY).await;
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    /// Offers `version` (or nothing), counting fetches and installs.
    #[derive(Clone, Default)]
    struct Fake {
        version: Option<&'static str>,
        fail: Option<&'static str>,
        fetches: Arc<AtomicUsize>,
        installs: Arc<AtomicUsize>,
        install_fails: bool,
    }

    impl Source for Fake {
        fn fetch<'a>(&'a self, found: Box<dyn FnOnce(&str) + Send + 'a>) -> Fetching<'a> {
            Box::pin(async move {
                self.fetches.fetch_add(1, Ordering::SeqCst);
                if let Some(message) = self.fail {
                    return Err(message.to_owned());
                }
                let Some(version) = self.version else {
                    return Ok(None);
                };
                found(version);
                let installs = self.installs.clone();
                let fails = self.install_fails;
                Ok(Some(Downloaded {
                    version: version.into(),
                    notes: "Faster OCR.".into(),
                    install: Box::new(move || {
                        installs.fetch_add(1, Ordering::SeqCst);
                        if fails {
                            Err("Vela.app couldn't be replaced.".into())
                        } else {
                            Ok(())
                        }
                    }),
                }))
            })
        }
    }

    fn at(hours: i64) -> DateTime<Utc> {
        DateTime::parse_from_rfc3339("2026-09-28T09:00:00Z")
            .unwrap()
            .with_timezone(&Utc)
            + Duration::hours(hours)
    }

    fn updates(fake: &Fake) -> (Updates, Arc<Mutex<Vec<UpdateStatus>>>) {
        let seen = Arc::new(Mutex::new(Vec::new()));
        let told = seen.clone();
        let updates = Updates::new(fake.clone(), move |s: &UpdateStatus| {
            told.lock().unwrap().push(s.clone())
        });
        (updates, seen)
    }

    #[tokio::test]
    async fn a_newer_version_is_downloaded_then_waits_to_be_installed() {
        let fake = Fake {
            version: Some("0.9.1"),
            ..Fake::default()
        };
        let (updates, seen) = updates(&fake);

        let status = updates.check(at(0)).await;

        let ready = UpdateStatus::Ready {
            version: "0.9.1".into(),
            notes: "Faster OCR.".into(),
        };
        assert_eq!(status, ready);
        assert_eq!(
            *seen.lock().unwrap(),
            [
                UpdateStatus::Checking,
                UpdateStatus::Downloading {
                    version: "0.9.1".into()
                },
                ready,
            ]
        );
        assert_eq!(updates.ready().as_deref(), Some("0.9.1"));
        assert_eq!(fake.installs.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn the_newest_version_says_when_it_was_checked() {
        let (updates, _) = updates(&Fake::default());
        assert_eq!(
            updates.check(at(0)).await,
            UpdateStatus::Idle {
                checked_at: Some("2026-09-28T09:00:00+00:00".into())
            }
        );
        assert_eq!(updates.ready(), None);
    }

    #[tokio::test]
    async fn checks_are_due_at_launch_then_every_six_hours() {
        let fake = Fake::default();
        let (updates, _) = updates(&fake);
        assert!(updates.due(at(0)));
        updates.check(at(0)).await;
        assert!(!updates.due(at(5)));
        assert!(updates.due(at(6)));
    }

    #[tokio::test]
    async fn once_downloaded_it_stops_checking() {
        let fake = Fake {
            version: Some("0.9.1"),
            ..Fake::default()
        };
        let (updates, _) = updates(&fake);
        updates.check(at(0)).await;
        assert!(!updates.due(at(24)));
        updates.check(at(24)).await;
        assert_eq!(fake.fetches.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn a_failed_check_is_shown_and_tried_again_later() {
        let fake = Fake {
            fail: Some("GitHub couldn't be reached."),
            ..Fake::default()
        };
        let (updates, _) = updates(&fake);
        assert_eq!(
            updates.check(at(0)).await,
            UpdateStatus::Failed {
                message: "GitHub couldn't be reached.".into()
            }
        );
        assert!(!updates.due(at(1)));
        assert!(updates.due(at(6)));
    }

    #[tokio::test]
    async fn installing_puts_the_download_in_place_once() {
        let fake = Fake {
            version: Some("0.9.1"),
            ..Fake::default()
        };
        let (updates, _) = updates(&fake);
        assert_eq!(updates.install(), Ok(false));
        updates.check(at(0)).await;

        assert_eq!(updates.install(), Ok(true));
        assert_eq!(
            updates.status(),
            UpdateStatus::Installing {
                version: "0.9.1".into()
            }
        );
        // Quitting after a restart was asked for installs nothing more.
        assert_eq!(updates.install(), Ok(false));
        assert_eq!(fake.installs.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn a_failed_install_leaves_the_update_ready() {
        let fake = Fake {
            version: Some("0.9.1"),
            install_fails: true,
            ..Fake::default()
        };
        let (updates, _) = updates(&fake);
        updates.check(at(0)).await;
        assert_eq!(
            updates.install(),
            Err("Vela.app couldn't be replaced.".into())
        );
        assert_eq!(updates.ready().as_deref(), Some("0.9.1"));
    }

    #[tokio::test]
    async fn quitting_drops_an_update_that_fails_to_install() {
        let fake = Fake {
            version: Some("0.9.1"),
            install_fails: true,
            ..Fake::default()
        };
        let (updates, _) = updates(&fake);
        updates.check(at(0)).await;
        assert!(updates.install_on_quit().is_err());
        // Nothing is left to hold up the quit a second time.
        assert_eq!(updates.ready(), None);
        assert_eq!(updates.install_on_quit(), Ok(false));
    }

    #[tokio::test]
    async fn a_development_build_never_checks() {
        let updates = Updates::unavailable();
        assert!(!updates.due(at(0)));
        assert_eq!(updates.check(at(0)).await, UpdateStatus::Unavailable);
        assert_eq!(updates.install(), Ok(false));
    }
}
