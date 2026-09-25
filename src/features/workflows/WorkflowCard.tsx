import type { WorkflowSummary } from "../../api/types";
import { Badge, Card } from "../../ui";
import styles from "./workflows.module.css";

export function WorkflowCard({ workflow: w }: { workflow: WorkflowSummary }) {
  return (
    <Card className={styles.card}>
      <div className={styles.cardHead}>
        <h3>{w.name}</h3>
        {w.needsYou > 0 && <Badge tone="danger">{w.needsYou} needs you</Badge>}
        <Badge tone={w.enabled ? "ok" : "neutral"}>{w.enabled ? "On" : "Off"}</Badge>
      </div>
      <p className={styles.muted}>{w.trigger}</p>
      <p className={styles.small}>Last run: {w.lastRun ?? "Never"}</p>
    </Card>
  );
}
