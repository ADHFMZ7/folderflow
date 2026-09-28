//! What the built Vela.app says about itself: its version, the oldest macOS
//! it runs on, how it's signed, and what macOS tells the person when Vela
//! first asks for a protected folder.

use std::path::Path;

fn manifest(rel: &str) -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join(rel)
}

fn json(rel: &str) -> serde_json::Value {
    serde_json::from_slice(&std::fs::read(manifest(rel)).unwrap()).unwrap()
}

#[test]
fn the_version_is_kept_in_package_json_and_the_crate_matches_it() {
    let conf = json("tauri.conf.json");
    assert_eq!(conf["version"], "../package.json");
    let package = json("../package.json");
    assert_eq!(package["version"], env!("CARGO_PKG_VERSION"));
}

#[test]
fn it_runs_on_macos_13_and_later() {
    let conf = json("tauri.conf.json");
    assert_eq!(conf["bundle"]["macOS"]["minimumSystemVersion"], "13.0");
}

/// Signed as a bundle, so its code signature names `com.adhfmz7.vela`. The
/// linker's own signature names the binary instead, and macOS then refuses
/// it notifications. Ad hoc until there's a Developer ID.
#[test]
fn the_bundle_is_signed_under_its_own_identifier() {
    let conf = json("tauri.conf.json");
    assert_eq!(conf["bundle"]["macOS"]["signingIdentity"], "-");
}

#[test]
fn macos_says_why_vela_asks_for_each_protected_place() {
    let info: plist::Dictionary = plist::from_file(manifest("Info.plist")).unwrap();
    for key in [
        "NSDesktopFolderUsageDescription",
        "NSDocumentsFolderUsageDescription",
        "NSDownloadsFolderUsageDescription",
        "NSRemovableVolumesUsageDescription",
        "NSNetworkVolumesUsageDescription",
    ] {
        let text = info
            .get(key)
            .and_then(|v| v.as_string())
            .unwrap_or_else(|| panic!("{key} is missing"));
        assert!(text.starts_with("Vela "), "{key}: {text}");
    }
}
