import type { Template } from "../../api/types";
import { TemplateCard } from "./TemplateCard";
import styles from "./workflows.module.css";

export function TemplatesPage({ templates }: { templates: Template[] }) {
  return (
    <div className={styles.page}>
      <header className={styles.head}><h1>Templates</h1></header>
      <div className={styles.grid}>{templates.map((t) => <TemplateCard key={t.id} template={t} />)}</div>
    </div>
  );
}
