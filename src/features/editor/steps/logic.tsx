// Settings for If, Stop and Ask me.

import { useState } from "react";
import type { Branch, ConditionOp } from "../../../api/types";
import { Segmented, Select, TextInput } from "../../../ui";
import { freshId } from "../graph";
import { useFieldProblems, useProblemsUnder, useStepContext } from "./context";
import { FormField, ListProblems, ListRows, Note, VariableText } from "./controls";
import type { FormProps } from "./triggers";
import { fillSample } from "./variables";
import styles from "./Steps.module.css";

const OPS: { op: ConditionOp; label: string }[] = [
  { op: "=", label: "is" }, { op: "!=", label: "is not" },
  { op: ">", label: "is more than" }, { op: "<", label: "is less than" },
  { op: ">=", label: "is at least" }, { op: "<=", label: "is at most" },
  { op: "contains", label: "contains" }, { op: "startsWith", label: "starts with" },
];
const COMPARING = new Set<ConditionOp>([">", "<", ">=", "<="]);

const CUSTOM = "__custom";
const VARIABLE = /^\{([A-Za-z0-9_]{1,32})\}$/;

export function IfForm({ step, onChange }: FormProps<"if">) {
  const { available } = useStepContext();
  const { condition } = step;
  const chosen = VARIABLE.exec(condition.left)?.[1];
  const detail = chosen ? available.find((v) => v.name === chosen) : undefined;
  const [custom, setCustom] = useState(!!condition.left && !detail);
  const set = (change: Partial<typeof condition>) => onChange({ ...step, condition: { ...condition, ...change } });

  const pickLeft = (value: string) => {
    if (value === CUSTOM) { setCustom(true); set({ left: detail ? "" : condition.left }); return; }
    setCustom(false);
    const v = available.find((a) => a.name === value);
    if (v?.kind === "yesNo") {
      set({ left: `{${value}}`, op: condition.op === "!=" ? "!=" : "=", right: condition.right === "no" ? "no" : "yes" });
    } else {
      set({ left: value ? `{${value}}` : "" });
    }
  };

  // Numbers and dates are usually compared; text is usually matched.
  const numeric = detail?.kind === "number" || detail?.kind === "date";
  const ops = detail?.kind === "yesNo" ? OPS.filter((o) => o.op === "=" || o.op === "!=")
    : numeric ? [...OPS.filter((o) => COMPARING.has(o.op)), ...OPS.filter((o) => !COMPARING.has(o.op))] : OPS;

  const leftProblems = useFieldProblems("condition.left");
  return (
    <div className={styles.sentence}>
      <FormField label="Compare" extraProblems={custom ? [] : leftProblems}>
        {({ id, describedBy }) => (
          <Select id={id} aria-describedby={describedBy} value={custom ? CUSTOM : chosen && detail ? chosen : ""}
            onChange={(e) => pickLeft(e.target.value)}>
            <option value="">Choose a detail…</option>
            {available.map((v) => <option key={v.name} value={v.name}>{v.label}</option>)}
            <option value={CUSTOM}>Custom text…</option>
          </Select>
        )}
      </FormField>
      {custom && <VariableText label="Custom text" field="condition.left" value={condition.left} onChange={(left) => set({ left })} />}
      <FormField label="Comparison">
        {({ id }) => (
          <Select id={id} value={condition.op} onChange={(e) => set({ op: e.target.value as ConditionOp })}>
            {ops.map((o) => <option key={o.op} value={o.op}>{o.label}</option>)}
          </Select>
        )}
      </FormField>
      {detail?.kind === "yesNo" && !custom ? (
        <div className={styles.field}>
          <span className={styles.label}>Value</span>
          <Segmented label="Value" value={condition.right === "no" ? "no" : "yes"} onChange={(right) => set({ right })}
            options={[{ value: "yes", label: "Yes" }, { value: "no", label: "No" }]} />
        </div>
      ) : (
        <VariableText label="Value" field="condition.right" value={condition.right} onChange={(right) => set({ right })}
          placeholder={numeric ? "A number, like 500" : "Text, like Screenshot"} />
      )}
      <p className={styles.hint}>Yes and No are the two exits on the card.</p>
    </div>
  );
}

export function StopForm() {
  return <Note>The run ends here, and nothing after it runs. A step with nothing after it ends the run too; Stop makes the ending obvious.</Note>;
}

export function AskMeForm({ step, onChange }: FormProps<"askMe">) {
  const { available } = useStepContext();
  const listProblems = useFieldProblems("answers");
  return (
    <>
      <VariableText label="Question" field="question" value={step.question} onChange={(question) => onChange({ ...step, question })}
        preview={step.question.trim() ? fillSample(step.question, available) : null}
        hint="The run waits here until you answer in FolderFlow." />
      <fieldset className={styles.section}>
        <legend className={styles.sectionTitle}>Answers</legend>
        <ListRows<Branch> items={step.answers} noun="answer" addLabel="Add an answer" min={2} keyOf={(a) => a.id}
          newItem={() => ({ id: freshId("a", step.answers.map((a) => a.id)), label: "" })}
          onChange={(answers) => onChange({ ...step, answers })}>
          {(a, i, set) => <AnswerRow answer={a} index={i} set={set} />}
        </ListRows>
        <ListProblems messages={listProblems} />
        <p className={styles.hint}>Each answer is an exit on the card. The steps after it can use your choice as {"{answer}"}.</p>
      </fieldset>
    </>
  );
}

function AnswerRow({ answer, index, set }: { answer: Branch; index: number; set: (a: Branch) => void }) {
  const problems = useProblemsUnder(`answers.${index}`);
  return (
    <>
      <TextInput aria-label={`Answer ${index + 1}`} value={answer.label} onChange={(e) => set({ ...answer, label: e.target.value })} />
      <ListProblems messages={problems} />
    </>
  );
}
