// A compact summary of which model each kind uses, for the sidebar or, inline,
// for a status bar.

import { useSettings } from "../../settings/SettingsProvider";
import { kindStatuses } from "./defaults";
import styles from "./models.module.css";

export function ModelReadiness({ inline = false }: { inline?: boolean }) {
  const { kinds, models, settings } = useSettings();
  return (
    <a className={inline ? styles.readinessInline : styles.readiness} href="#/settings" aria-label="Models">
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
