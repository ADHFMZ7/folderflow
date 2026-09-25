// Tells the user when the settings they see aren't what they last saved.

import { useSettings } from "./SettingsProvider";
import styles from "./SettingsNoticeBanner.module.css";

export function SettingsNoticeBanner() {
  const { notice, dismissNotice } = useSettings();
  if (!notice) return null;
  return (
    <div className={styles.banner} role="alert">
      <p>
        <strong>Your settings file was damaged</strong>, so FolderFlow started with fresh settings. The old file was kept at{" "}
        <code className={styles.path}>{notice.backup}</code>.
      </p>
      <button type="button" className={styles.dismiss} onClick={dismissNotice}>Dismiss</button>
    </div>
  );
}
