// "Run…": runs the saved workflow on files the person picks, and says
// in the toolbar how it went. The run history comes with its own page later.

import { useEffect, useRef, useState } from "react";
import { Play } from "lucide-react";
import { useApi } from "../../api/api";
import { ApiError, type RunStatus, type RunSummary, type Workflow } from "../../api/types";
import { Button } from "../../ui";
import styles from "./Studio.module.css";

const FINISHED: RunStatus[] = ["done", "failed", "interrupted", "undone"];

type Props = {
  workflow: Workflow;
  problems: number;
  /** Saves pending edits first, so the run uses them. False if the save failed. */
  flush: () => Promise<boolean>;
};

export function RunNow({ workflow, problems, flush }: Props) {
  const api = useApi();
  const [status, setStatus] = useState<string | null>(null);
  const stopListening = useRef<(() => void) | null>(null);
  useEffect(() => () => stopListening.current?.(), []);

  const trigger = workflow.steps.find((s) => s.type === "fileAdded" || s.type === "runNow" || s.type === "schedule");
  if (!trigger || trigger.type === "schedule") return null;

  async function run() {
    if (!(await flush())) return;
    const files = await api.chooseFiles(trigger?.type === "fileAdded" ? trigger.folder : undefined);
    if (!files.length) return;

    stopListening.current?.();
    // Listen before queuing: a quick run can end before runNow answers.
    const finished = new Set<string>();
    let queued: RunSummary[] | null = null;
    let names: string[] = [];
    let settled = false;
    const settle = async () => {
      if (settled || !queued || !queued.every((r) => finished.has(r.id))) return;
      settled = true;
      stop();
      const runs = await Promise.all(queued.map((r) => api.getRun(r.id)));
      setStatus(outcome(names, runs.map((r) => (r.status === "done" ? "" : r.error?.message ?? `The run was ${r.status}.`))));
    };
    const stop = api.onRunChanged((change) => {
      if (!FINISHED.includes(change.status)) return;
      finished.add(change.runId);
      void settle();
    });
    stopListening.current = stop;

    try {
      queued = await api.runNow(workflow.id, files);
    } catch (e) {
      stop();
      setStatus(e instanceof ApiError ? e.message : "Couldn't start the run.");
      return;
    }
    names = queued.map((r) => r.file ?? "a file");
    setStatus(`Running on ${count(names)}…`);
    void settle();
  }

  return (
    <>
      {status && <span className={styles.runStatus} role="status" title={status}>{status}</span>}
      <Button variant="secondary" onClick={run} disabled={problems > 0}
        title={problems ? `Fix ${problems === 1 ? "the problem" : `${problems} problems`} first` : "Run this workflow on files you choose"}>
        <Play size={14} strokeWidth={2} aria-hidden /> Run…
      </Button>
    </>
  );
}

const count = (names: string[]) => (names.length === 1 ? names[0] : `${names.length} files`);

/** "Ran on a.pdf", "Failed on a.pdf: why", or "Ran on 3 files, 1 failed: why". Errors are "" for a run that went fine. */
function outcome(names: string[], errors: string[]) {
  const failed = errors.filter(Boolean);
  if (!failed.length) return `Ran on ${count(names)}`;
  if (names.length === 1) return `Failed on ${names[0]}: ${failed[0]}`;
  return `Ran on ${names.length} files, ${failed.length} failed: ${failed[0]}`;
}
