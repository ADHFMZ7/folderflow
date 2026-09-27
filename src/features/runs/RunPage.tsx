// One run: what each step did, in plain words, what it's waiting for if
// anything, and Undo run.

import { useCallback, useEffect, useState } from "react";
import { useApi } from "../../api/api";
import { ApiError, type NeedsYouItem, type Run, type StepRun } from "../../api/types";
import { hrefFor } from "../../app/routes";
import { Badge, Banner, Button, Spinner } from "../../ui";
import { ago, STATUS_WORDS, statusTone, undoWords } from "./format";
import { RunActions } from "./RunActions";
import { useRunChanged } from "./useRunChanged";
import styles from "./runs.module.css";

const TRIGGERS = { fileAdded: "A file was added", schedule: "Its schedule", runNow: "You chose Run" } as const;

export function RunPage({ id }: { id: string }) {
  const api = useApi();
  const [run, setRun] = useState<Run | null>(null);
  const [needs, setNeeds] = useState<NeedsYouItem | null>(null);
  const [missing, setMissing] = useState<string | null>(null);
  const [said, setSaid] = useState<string | null>(null);
  const [confirming, setConfirming] = useState(false);

  const refresh = useCallback(async () => {
    try {
      const [r, items] = await Promise.all([api.getRun(id), api.listNeedsYou()]);
      setRun(r);
      setNeeds(items.find((i) => i.run.id === id) ?? null);
    } catch (e) {
      setMissing(e instanceof ApiError ? e.message : String(e));
    }
  }, [api, id]);
  useEffect(() => { void refresh(); }, [refresh]);
  useRunChanged((change) => { if (change.runId === id) void refresh(); });

  const back = <a className={styles.back} href={hrefFor({ page: "history" })}>History</a>;
  if (missing) return <div className={styles.page}>{back}<p>{missing}</p></div>;
  if (!run) return <Spinner label="Opening run…" />;

  const name = run.trigger.file?.path.split("/").pop() ?? run.workflow.name;
  const undoable = run.status === "done" || run.status === "failed" || run.status === "interrupted";

  async function undo() {
    setConfirming(false);
    try {
      setSaid(undoWords(await api.undoRun(id)));
    } catch (e) {
      setSaid(e instanceof ApiError ? e.message : String(e));
    }
    void refresh();
  }

  return (
    <div className={styles.page}>
      {back}
      <header className={styles.head}>
        <div className={styles.title}>
          <h1>{name}</h1>
          <span className={styles.muted}>
            <a className={styles.link} href={hrefFor({ page: "workflow", id: run.workflowId })}>{run.workflow.name}</a>
            {" · "}{TRIGGERS[run.trigger.kind]} · {ago(run.startedAt)}
          </span>
        </div>
        <Badge tone={statusTone(run.status)}>{STATUS_WORDS[run.status]}</Badge>
        {undoable && !confirming && (
          <Button variant="secondary" onClick={() => setConfirming(true)}>Undo run</Button>
        )}
      </header>
      {confirming && (
        <div className={styles.confirm} role="group" aria-label="Undo this run?">
          <span>Put back every change this run made? Files changed since are left alone.</span>
          <Button onClick={undo}>Undo run</Button>
          <Button variant="secondary" onClick={() => setConfirming(false)}>Cancel</Button>
        </div>
      )}
      {said && <Banner tone="info">{said}</Banner>}
      {run.undo && <UndoResult run={run} />}
      {needs && (
        <section className={styles.waiting} aria-label="Needs you">
          <p className={needs.kind === "question" ? styles.question : undefined}>{needs.message}</p>
          <RunActions item={needs} onResult={(m) => { setSaid(m); void refresh(); }} />
        </section>
      )}
      <ol className={styles.steps} aria-label="Steps">
        {run.steps.map((s, i) => <StepLine key={i} step={s} run={run} />)}
      </ol>
    </div>
  );
}

function UndoResult({ run }: { run: Run }) {
  const undo = run.undo!;
  return (
    <section className={styles.undone} aria-label="Undone">
      <p>This run was undone. {undoWords(undo)}</p>
      {undo.leftAlone.length > 0 && (
        <ul>{undo.leftAlone.map((l) => <li key={l.path}>{l.reason}</li>)}</ul>
      )}
    </section>
  );
}

const OUTCOME: Record<StepRun["outcome"], string> = { running: "Running", waiting: "Waiting", done: "Done", failed: "Failed" };

/** One step as it ran: its title, and what it did or the branch it took. */
function StepLine({ step, run }: { step: StepRun; run: Run }) {
  const said = step.message ?? branchWords(step, run);
  return (
    <li className={styles[`step-${step.outcome}`]}>
      <span className={styles.stepTitle}>{step.title}</span>
      <span className={styles.outcome}>{OUTCOME[step.outcome]}</span>
      {said && <span className={step.outcome === "failed" ? styles.error : styles.muted}>{said}</span>}
    </li>
  );
}

/** "Went the Yes way", with the branch's own label. */
function branchWords(step: StepRun, run: Run): string | null {
  if (!step.branch) return null;
  const def = run.workflow.steps.find((s) => s.id === step.stepId);
  const labels = def?.type === "askMe" ? def.answers : def?.type === "classify" ? def.categories : [];
  const label = labels.find((b) => b.id === step.branch)?.label
    ?? (step.branch === "yes" ? "Yes" : step.branch === "no" ? "No" : step.branch);
  return `Went the ${label} way.`;
}
