import type { Template } from "../../api/types";
import { Card } from "../../ui";
import styles from "./workflows.module.css";

export function TemplateCard({ template: t }: { template: Template }) {
  return (
    <Card className={styles.card}>
      <h3>{t.name}</h3>
      <p className={styles.muted}>{t.blurb}</p>
      <p className={styles.trigger}>{t.trigger}</p>
    </Card>
  );
}
