// A compact summary of which model each kind uses, for the sidebar.

import { useSettings } from "../../settings/SettingsProvider";
import { kindStatuses } from "./defaults";
import styles from "./models.module.css";

export function ModelReadiness() {
  const { kinds, models, settings } = useSettings();
  return (
    <a className={styles.readiness} href="#/settings" aria-label="Models">
      {kindStatuses(kinds, settings.defaults, models).map(({ kind, model }) => (
        <span key={kind.id} className={styles.readinessRow}>
          <span className={model ? styles.dotOk : styles.dotWarn} aria-hidden />
          <span className={styles.readinessKind}>{kind.name}</span>
          <span className={styles.muted}>{model ? model.name : "Not set"}</span>
        </span>
      ))}
    </a>
  );
}
