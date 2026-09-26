import type { Template } from "../../api/types";
import styles from "./workflows.module.css";

/** A template; choosing it creates a workflow from it. */
export function TemplateCard({ template: t, onUse }: { template: Template; onUse: (id: string) => void }) {
  return (
    <button type="button" className={styles.template} onClick={() => onUse(t.id)}>
      <strong className={styles.templateName}>{t.name}</strong>
      <span className={styles.muted}>{t.blurb}</span>
      <span className={styles.trigger}>{t.trigger}</span>
    </button>
  );
}
