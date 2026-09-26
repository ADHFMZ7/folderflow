import { useSettings } from "../../settings/SettingsProvider";
import { Segmented, Toggle } from "../../ui";
import { ModelSetup } from "../models/ModelSetup";
import styles from "./SettingsPage.module.css";

export function SettingsPage() {
  const { settings, update } = useSettings();
  return (
    <div className={styles.page}>
      <h1>Settings</h1>
      <section className={styles.section} aria-labelledby="models-title">
        <h2 id="models-title">Models</h2>
        <p className={styles.muted}>Connect providers, then choose which model each kind of step uses.</p>
        <ModelSetup />
      </section>
      <section className={styles.section} aria-labelledby="general-title">
        <h2 id="general-title">General</h2>
        <div className={styles.row}>
          <span>Appearance</span>
          <Segmented label="Appearance" value={settings.appearance} onChange={(appearance) => update({ appearance })}
            options={[{ value: "system", label: "Match system" }, { value: "light", label: "Light" }, { value: "dark", label: "Dark" }]} />
        </div>
        <Toggle label="Open FolderFlow when I log in" checked={settings.openAtLogin} onChange={(openAtLogin) => update({ openAtLogin })} />
      </section>
    </div>
  );
}
