import { useSettings } from "../settings/SettingsProvider";
import { FirstLaunch } from "../features/first-launch/FirstLaunch";
import { MainWindow } from "../features/shell/MainWindow";

/** First launch until setup is complete, then the main window. */
export function Root() {
  const { settings } = useSettings();
  return settings.setupComplete ? <MainWindow /> : <FirstLaunch />;
}
