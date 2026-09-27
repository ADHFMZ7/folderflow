// History: every run, newest first, narrowed by workflow or state. Each row
// opens the run's page.

import { useCallback, useEffect, useState } from "react";
import { useApi } from "../../api/api";
import type { RunStatus, RunSummary, WorkflowSummary } from "../../api/types";
import { hrefFor } from "../../app/routes";
import { Badge, Button, Select, Spinner } from "../../ui";
import { ago, runName, STATUS_WORDS, statusTone } from "./format";
import { useRunChanged } from "./useRunChanged";
import styles from "./runs.module.css";

const PAGE = 50;
const STATUSES = Object.keys(STATUS_WORDS) as RunStatus[];

export function HistoryPage() {
  const api = useApi();
  const [workflows, setWorkflows] = useState<WorkflowSummary[]>([]);
  const [workflowId, setWorkflowId] = useState("");
  const [status, setStatus] = useState<RunStatus | "">("");
  const [runs, setRuns] = useState<RunSummary[] | null>(null);
  const [more, setMore] = useState(false);

  const query = useCallback((before?: string) => api.listRuns({
    workflowId: workflowId || undefined, status: status || undefined, before, limit: PAGE,
  }), [api, workflowId, status]);

  // Reloads as many runs as are shown, so a change doesn't lose the pages below.
  const refresh = useCallback(async (count = PAGE) => {
    const list = await api.listRuns({ workflowId: workflowId || undefined, status: status || undefined, limit: count });
    setRuns(list);
    setMore(list.length === count);
  }, [api, workflowId, status]);

  useEffect(() => { void refresh(); }, [refresh]);
  useEffect(() => { api.listWorkflows().then((w) => setWorkflows(w.filter((x) => x.status === "ok"))); }, [api]);
  useRunChanged(() => { void refresh(Math.max(PAGE, runs?.length ?? 0)); });

  async function showMore() {
    if (!runs?.length) return;
    const next = await query(runs[runs.length - 1].id);
    setRuns([...runs, ...next]);
    setMore(next.length === PAGE);
  }

  return (
    <div className={styles.page}>
      <header className={styles.head}>
        <h1>History</h1>
        <div className={styles.filters}>
          <Select aria-label="Workflow" value={workflowId} onChange={(e) => setWorkflowId(e.target.value)}>
            <option value="">All workflows</option>
            {workflows.map((w) => <option key={w.id} value={w.id}>{w.name}</option>)}
          </Select>
          <Select aria-label="State" value={status} onChange={(e) => setStatus(e.target.value as RunStatus | "")}>
            <option value="">Any state</option>
            {STATUSES.map((s) => <option key={s} value={s}>{STATUS_WORDS[s]}</option>)}
          </Select>
        </div>
      </header>
      {runs === null ? <Spinner label="Loading runs…" />
        : !runs.length ? (
          <p className={styles.muted}>
            {workflowId || status ? "No runs match." : "No runs yet. Runs appear here as your workflows work on files."}
          </p>
        ) : (
          <>
            <ul className={styles.rows} aria-label="Runs">
              {runs.map((r) => (
                <li key={r.id}>
                  <a className={styles.row} href={hrefFor({ page: "run", id: r.id })}>
                    <span className={styles.rowMain}>
                      <span className={styles.rowName}>{runName(r)}</span>
                      <span className={styles.muted}>{r.workflowName} · {ago(r.startedAt)}</span>
                      {r.error && r.status === "failed" && <span className={styles.error}>{r.error}</span>}
                    </span>
                    <Badge tone={statusTone(r.status)}>{STATUS_WORDS[r.status]}</Badge>
                  </a>
                </li>
              ))}
            </ul>
            {more && <Button variant="secondary" onClick={showMore}>Show more</Button>}
          </>
        )}
    </div>
  );
}
