// Try on a file: runs the workflow as it stands in the editor on one chosen
// file, changing nothing, and shows what each step would do. Questions are
// answered inside the try. See docs/engine.md, "Try on a file".

import { useCallback, useState } from "react";
import { Check, CircleHelp, X } from "lucide-react";
import { useApi } from "../../api/api";
import { ApiError, type TryResult, type Workflow } from "../../api/types";
import type { Tried } from "./graph";
import { Button, Spinner } from "../../ui";
import styles from "./Studio.module.css";

export type Trial = {
  file: string;
  answers: Record<string, string>;
  /** The draft the result is for: the workflow may have changed since. */
  of: Workflow;
  result: TryResult | null;
  error: string | null;
  busy: boolean;
};

/** Starting, answering and repeating a try of `draft`. */
export function useTry(draft: Workflow) {
  const api = useApi();
  const [trial, setTrial] = useState<Trial | null>(null);

  const run = useCallback(async (file: string, answers: Record<string, string>) => {
    setTrial((t) => ({ file, answers, of: draft, result: t?.file === file ? t.result : null, error: null, busy: true }));
    try {
      const result = await api.tryOnFile(draft, file, answers);
      setTrial({ file, answers, of: draft, result, error: null, busy: false });
    } catch (e) {
      const error = e instanceof ApiError ? e.message : "Couldn't try the workflow.";
      setTrial({ file, answers, of: draft, result: null, error, busy: false });
    }
  }, [api, draft]);

  /** Asks for a file and tries on it; `onChosen` runs once one is chosen. */
  const start = useCallback(async (onChosen?: () => void) => {
    const trigger = draft.steps.find((s) => s.type === "fileAdded");
    const [file] = await api.chooseFiles(trigger?.type === "fileAdded" ? trigger.folder : undefined);
    if (!file) return;
    onChosen?.();
    await run(file, {});
  }, [api, draft, run]);

  return {
    trial,
    start,
    again: () => trial && run(trial.file, {}),
    answer: (stepId: string, branchId: string) => trial && run(trial.file, { ...trial.answers, [stepId]: branchId }),
    close: () => setTrial(null),
  };
}

/** What the try did at each step it reached, for the canvas. */
export function triedSteps(trial: Trial | null): Record<string, Tried> {
  return Object.fromEntries((trial?.result?.steps ?? []).map((s) => [s.stepId, { outcome: s.outcome, branch: s.branch }]));
}

type PanelProps = ReturnType<typeof useTry> & {
  trial: Trial;
  draft: Workflow;
  onSelect: (stepId: string) => void;
};

export function TryPanel({ trial, draft, again, start, answer, close, onSelect }: PanelProps) {
  const name = trial.file.slice(trial.file.lastIndexOf("/") + 1);
  const { result } = trial;
  const titleOf = (stepId: string | null) => draft.steps.find((s) => s.id === stepId)?.title ?? "a step";

  return (
    <section className={`${styles.inspector} ${styles.try}`} aria-label={`Try on ${name}`}>
      <div className={styles.cardHead}>
        <h3>Try on {name}</h3>
        <button type="button" className={styles.close} onClick={close} aria-label="Close the try" title="Close">
          <X size={16} strokeWidth={2} aria-hidden />
        </button>
      </div>

      {trial.of !== draft && !trial.busy && <p className={styles.muted}>The workflow changed since this try.</p>}
      {trial.busy && <Spinner label="Trying…" />}
      {trial.error && <p className={styles.tryStopped} role="alert">{trial.error}</p>}
      {result?.status === "done" && <p>Nothing was changed. This is what a run would do.</p>}
      {result?.status === "failed" && (
        <p className={styles.tryStopped}>Stopped at {titleOf(result.error?.stepId ?? null)}: {result.error?.message}</p>
      )}
      {result?.status === "waiting" && result.question && (
        <div className={styles.tryQuestion}>
          <p>{result.question.question}</p>
          <div className={styles.tryAnswers}>
            {result.question.answers.map((a) => (
              <Button key={a.id} variant="secondary" disabled={trial.busy}
                onClick={() => answer(result.question!.stepId, a.id)}>{a.label}</Button>
            ))}
          </div>
        </div>
      )}

      {result && (
        <ol className={styles.trySteps}>
          {result.steps.map((step, i) => <TriedStep key={`${step.stepId}-${i}`} step={step} onSelect={onSelect} />)}
        </ol>
      )}

      <div className={styles.tryActions}>
        <Button variant="secondary" onClick={again} disabled={trial.busy}>Try again</Button>
        <Button variant="secondary" onClick={() => void start()} disabled={trial.busy}>Another file…</Button>
      </div>
    </section>
  );
}

function TriedStep({ step, onSelect }: { step: TryResult["steps"][number]; onSelect: (stepId: string) => void }) {
  const [open, setOpen] = useState(false);
  const values = Object.entries(step.values);
  return (
    <li className={styles.tryStep} data-outcome={step.outcome}>
      <span className={styles.tryGlyph} aria-hidden>
        {step.outcome === "done" ? <Check size={14} strokeWidth={2.4} />
          : step.outcome === "waiting" ? <CircleHelp size={14} strokeWidth={2.2} /> : <X size={14} strokeWidth={2.4} />}
      </span>
      <div className={styles.tryBody}>
        <button type="button" className={styles.tryTitle} onClick={() => onSelect(step.stepId)}>{step.title}</button>
        {step.message && <p>{step.message}</p>}
        {values.length > 0 && (
          <>
            <button type="button" className={styles.tryValuesToggle} aria-expanded={open}
              aria-label={`Values of ${step.title}`} onClick={() => setOpen(!open)}>
              {open ? "Hide values" : `${values.length} value${values.length === 1 ? "" : "s"}`}
            </button>
            {open && (
              <dl className={styles.tryValues}>
                {values.map(([k, v]) => <div key={k}><dt>{k}</dt><dd>{v.value}</dd></div>)}
              </dl>
            )}
          </>
        )}
      </div>
    </li>
  );
}
