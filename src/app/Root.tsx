import { FirstLaunch } from "../features/first-launch/FirstLaunch";
import { MainWindow } from "../features/shell/MainWindow";
import { SettingsNoticeBanner } from "../settings/SettingsNoticeBanner";
import { useSettings } from "../settings/SettingsProvider";
import styles from "./Root.module.css";

/** First launch until setup is complete, then the main window. */
export function Root() {
  const { settings } = useSettings();
  return (
    <div className={styles.root}>
      <SettingsNoticeBanner />
      <div className={styles.view}>
        {settings.setupComplete ? <MainWindow /> : (
          <>
            {/* First launch has no top bar; this strip still moves the window. */}
            <div className="window-drag-strip" data-tauri-drag-region />
            <FirstLaunch />
          </>
        )}
      </div>
    </div>
  );
}
