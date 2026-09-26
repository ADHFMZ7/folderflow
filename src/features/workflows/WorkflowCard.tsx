import { useState } from "react";
import type { WorkflowSummary } from "../../api/types";
import { hrefFor } from "../../app/routes";
import { Badge, Button, Card } from "../../ui";
import styles from "./workflows.module.css";

export function WorkflowCard({ workflow: w, onDelete }: { workflow: WorkflowSummary; onDelete: (id: string) => Promise<void> }) {
  const [confirming, setConfirming] = useState(false);

  if (w.status !== "ok") {
    return (
      <Card className={styles.card} role="article" aria-label={w.name}>
        <h3 className={styles.fileName}>{w.name}</h3>
        <p className={styles.muted}>
          {w.status === "damaged"
            ? "This workflow file can't be read. FolderFlow left it untouched."
            : "This workflow was saved by a newer FolderFlow. Open the newer version to change it."}
        </p>
      </Card>
    );
  }

  return (
    <Card className={styles.card} role="article" aria-label={w.name}>
      <a className={styles.cardLink} href={hrefFor({ page: "workflow", id: w.id })}>
        <span className={styles.cardHead}>
          <h3>{w.name}</h3>
          {w.needsYou > 0 && <Badge tone="danger">{w.needsYou} needs you</Badge>}
          <Badge tone={w.enabled ? "ok" : "neutral"}>{w.enabled ? "On" : "Off"}</Badge>
        </span>
        <span className={styles.muted}>{w.trigger}</span>
        <span className={styles.small}>Last run: {w.lastRun ?? "Never"}</span>
      </a>
      {confirming ? (
        <div className={styles.confirm}>
          <span>Move this workflow to the trash?</span>
          <Button variant="danger" onClick={() => onDelete(w.id)}>Move to trash</Button>
          <Button variant="secondary" onClick={() => setConfirming(false)}>Cancel</Button>
        </div>
      ) : (
        <Button variant="quiet" className={styles.delete} aria-label={`Delete ${w.name}`} onClick={() => setConfirming(true)}>Delete</Button>
      )}
    </Card>
  );
}
