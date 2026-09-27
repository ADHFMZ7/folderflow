// Needs you, at the top of the Workflows page: questions waiting for an answer,
// and failed and stopped runs waiting for a decision. See docs/engine.md.

import { useCallback, useEffect, useState } from "react";
import { useApi } from "../../api/api";
import type { NeedsYouItem } from "../../api/types";
import { hrefFor } from "../../app/routes";
import { Banner, Card } from "../../ui";
import { ago, runName } from "./format";
import { RunActions } from "./RunActions";
import { useRunChanged } from "./useRunChanged";
import styles from "./runs.module.css";

const HEADINGS: Record<NeedsYouItem["kind"], string> = { question: "Question", failed: "Failed", interrupted: "Stopped" };

export function NeedsYou() {
  const api = useApi();
  const [items, setItems] = useState<NeedsYouItem[]>([]);
  const [said, setSaid] = useState<string | null>(null);
  const refresh = useCallback(async () => setItems(await api.listNeedsYou()), [api]);
  useEffect(() => { void refresh(); }, [refresh]);
  useRunChanged(() => { void refresh(); });

  if (!items.length && !said) return null;
  return (
    <section className={styles.needsYou} aria-label="Needs you">
      <h2>Needs you</h2>
      {said && <Banner tone="info">{said}</Banner>}
      <ul className={styles.list}>
        {items.map((item) => (
          <li key={item.run.id}>
            <Card className={styles.item} role="article" aria-label={`${item.run.workflowName}: ${runName(item.run)}`}>
              <div className={styles.itemHead}>
                <span className={styles.kind}>{HEADINGS[item.kind]}</span>
                <span className={styles.muted}>{item.run.workflowName} · {runName(item.run)} · {ago(item.run.startedAt)}</span>
                <a className={styles.link} href={hrefFor({ page: "run", id: item.run.id })}>Details</a>
              </div>
              <p className={item.kind === "question" ? styles.question : undefined}>
                {item.kind === "failed" && item.step ? `${item.step}: ` : ""}{item.message}
              </p>
              <RunActions item={item} onResult={(message) => { setSaid(message); void refresh(); }} />
            </Card>
          </li>
        ))}
      </ul>
    </section>
  );
}
