// The right-hand panel: the selected step, or the workflow's problems when
// nothing is selected. Per-type settings forms arrive in the next change.

import type { Problem, Step } from "../../api/types";
import { Button, Field, TextInput } from "../../ui";
import { infoFor } from "./catalog";
import styles from "./Studio.module.css";

type Props = {
  step: Step | null;
  problems: Problem[];
  stepTitle: (id: string) => string | null;
  onChange: (step: Step) => void;
  onDelete: (id: string) => void;
  onSelect: (id: string) => void;
};

export function Inspector({ step, problems, stepTitle, onChange, onDelete, onSelect }: Props) {
  if (!step) {
    return (
      <aside className={styles.inspector} aria-label="Inspector">
        <h3>Problems</h3>
        {problems.length === 0 ? <p className={styles.note}>No problems. Select a step to edit it.</p> : (
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
      </aside>
    );
  }

  const own = problems.filter((p) => p.stepId === step.id);
  return (
    <aside className={styles.inspector} aria-label="Inspector">
      <p className={styles.note}>{infoFor(step.type).label}</p>
      <Field label="Title">
        <TextInput value={step.title} onChange={(e) => onChange({ ...step, title: e.target.value })} />
      </Field>
      {own.length > 0 && (
        <ul className={styles.problems} aria-label="Problems with this step">
          {own.map((p, i) => <li key={i} className={styles.problem}><span>{p.message}</span></li>)}
        </ul>
      )}
      <p className={styles.note}>Settings for this kind of step are coming in the next update.</p>
      <Button variant="danger" onClick={() => onDelete(step.id)}>Delete step</Button>
    </aside>
  );
}
