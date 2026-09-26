import type { Template } from "../../api/types";
import { TemplateCard } from "./TemplateCard";
import styles from "./workflows.module.css";

export function TemplatesPage({ templates, onUse }: { templates: Template[]; onUse: (id: string) => void }) {
  return (
    <div className={styles.page}>
      <header className={styles.head}><h1>Templates</h1></header>
      <p className={styles.muted}>Pick one to make your own copy, then change it however you like.</p>
      <div className={styles.grid}>{templates.map((t) => <TemplateCard key={t.id} template={t} onUse={onUse} />)}</div>
    </div>
  );
}
