// The card that floats over the right of the canvas: the selected step's settings,
// or the workflow's problems when those are asked for. Every edit is reported as
// one whole new step through onChange. See docs/step-settings.md.

import { useMemo, type ReactNode } from "react";
import { X } from "lucide-react";
import type { Problem, Step, Workflow } from "../../api/types";
import { Button, Field, TextInput } from "../../ui";
import { infoFor, KIND_LABEL } from "./catalog";
import { isTrigger } from "./graph";
import { isUnder, StepContext, type StepContextValue } from "./steps/context";
import { RunsOn } from "./steps/RunsOn";
import { SHOWN_FIELDS, StepForm } from "./steps/StepForm";
import { retitle } from "./steps/titles";
import { availableAt } from "./steps/variables";
import styles from "./Studio.module.css";
import stepStyles from "./steps/Steps.module.css";

type Props = {
  workflow: Workflow;
  /** The step to show; null shows the problems. */
  step: Step | null;
  problems: Problem[];
  stepTitle: (id: string) => string | null;
  onChange: (step: Step) => void;
  onDelete: (id: string) => void;
  onSelect: (id: string) => void;
  onClose: () => void;
};

function Card({ title, onClose, children }: { title: ReactNode; onClose: () => void; children: ReactNode }) {
  return (
    <aside className={styles.inspector} aria-label="Inspector">
      <div className={styles.cardHead}>
        {title}
        <button type="button" className={styles.close} aria-label="Close" title="Close" onClick={onClose}>
          <X size={16} strokeWidth={1.8} aria-hidden />
        </button>
      </div>
      {children}
    </aside>
  );
}

export function Inspector({ workflow, step, problems, stepTitle, onChange, onDelete, onSelect, onClose }: Props) {
  if (!step) {
    return (
      <Card title={<h3>Problems</h3>} onClose={onClose}>
        {problems.length === 0 ? <p className={styles.note}>Nothing to fix.</p> : (
          <ul className={styles.problems} aria-label="Problems">
            {problems.map((p, i) => (
              <li key={i}>
                {p.stepId ? (
                  <button type="button" className={styles.problem} onClick={() => onSelect(p.stepId!)}>
                    <strong>{stepTitle(p.stepId) ?? "A step"}</strong><span>{p.message}</span>
                  </button>
                ) : <div className={styles.problem}><strong>Workflow</strong><span>{p.message}</span></div>}
              </li>
            ))}
          </ul>
        )}
      </Card>
    );
  }
  return <StepSettings key={step.id} workflow={workflow} step={step} problems={problems} onChange={onChange} onDelete={onDelete} onClose={onClose} />;
}

function StepSettings({ workflow, step, problems, onChange, onDelete, onClose }: Pick<Props, "workflow" | "problems" | "onChange" | "onDelete" | "onClose"> & { step: Step }) {
  const info = infoFor(step.type);
  const own = problems.filter((p) => p.stepId === step.id);
  const shown = SHOWN_FIELDS[step.type];
  const elsewhere = own.filter((p) => !p.field || !shown.some((f) => isUnder(p.field!, f)));

  const available = useMemo(() => availableAt(workflow, step.id), [workflow, step.id]);
  const context: StepContextValue = useMemo(
    () => ({ available, reachable: isTrigger(step.type) || available.length > 0, problems: own }),
    // `own` is new on every render; its contents only change with `problems`.
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [available, step.type, problems, step.id],
  );

  return (
    <Card onClose={onClose} title={
      <p className={stepStyles.kindLine}>
        <span className={`${styles.glyph} ${styles[`kind-${info.kind}`]}`} aria-hidden>{info.glyph}</span>
        {KIND_LABEL[info.kind]} · {info.label}
      </p>}>
      <Field label="Title">
        <TextInput value={step.title} onChange={(e) => onChange({ ...step, title: e.target.value })} />
      </Field>
      {elsewhere.length > 0 && (
        <ul className={styles.problems} aria-label="Problems with this step">
          {elsewhere.map((p, i) => <li key={i} className={styles.problem}><span>{p.message}</span></li>)}
        </ul>
      )}
      <StepContext.Provider value={context}>
        <div className={stepStyles.form}>
          <StepForm step={step} onChange={(edited) => onChange(retitle(step, edited))} />
        </div>
      </StepContext.Provider>
      <RunsOn type={step.type} />
      <Button variant="danger" onClick={() => onDelete(step.id)}>Delete step</Button>
    </Card>
  );
}
