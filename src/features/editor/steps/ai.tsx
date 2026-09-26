// Settings for the AI steps: Classify, Extract, Write and Agent. Each asks, in
// plain words, for what makes a model do the job well: what each category
// looks like, what each detail is, how long and in what tone to write.

import type { Category, Field, FieldType } from "../../../api/types";
import { Button, Segmented, Select, TextArea, TextInput } from "../../../ui";
import { freshId } from "../graph";
import { useFieldProblems, useProblemsUnder } from "./context";
import { FormField, ListProblems, ListRows, NameInput, VariableText } from "./controls";
import type { FormProps } from "./triggers";
import styles from "./Steps.module.css";

const CATCH_ALL = /\b(else|other|anything|misc|unknown|rest)\b/i;

/** Sets an optional text: empty removes it, so files stay tidy. */
function withDescription<T extends { description?: string }>(item: T, text: string): T {
  const { description: _, ...rest } = item;
  return (text ? { ...rest, description: text } : rest) as T;
}

export function ClassifyForm({ step, onChange }: FormProps<"classify">) {
  const listProblems = useFieldProblems("categories");
  const hasCatchAll = step.categories.some((c) => CATCH_ALL.test(c.label));
  return (
    <>
      <fieldset className={styles.section}>
        <legend className={styles.sectionTitle}>Categories</legend>
        <p className={styles.hint}>Each category is an exit on the card. Say what belongs in it, so the model can tell them apart.</p>
        <ListRows<Category> items={step.categories} noun="category" addLabel="Add a category" min={2}
          keyOf={(c) => c.id}
          newItem={() => ({ id: freshId("c", step.categories.map((c) => c.id)), label: "" })}
          onChange={(categories) => onChange({ ...step, categories })}>
          {(c, i, set) => <CategoryRow category={c} index={i} set={set} />}
        </ListRows>
        <ListProblems messages={listProblems} />
        {!hasCatchAll && (
          <p className={styles.hint}>Tip: Add one like "Something else", so files that don't fit aren't forced into a category.</p>
        )}
      </fieldset>
      <VariableText label="Anything else it should know" field="instructions" multiline value={step.instructions}
        placeholder="Optional, e.g. These come from my work email."
        onChange={(instructions) => onChange({ ...step, instructions })} />
    </>
  );
}

function CategoryRow({ category, index, set }: { category: Category; index: number; set: (c: Category) => void }) {
  const problems = useProblemsUnder(`categories.${index}`);
  const n = index + 1;
  return (
    <>
      <TextInput aria-label={`Category ${n} name`} placeholder="Name, like Receipt" value={category.label}
        onChange={(e) => set({ ...category, label: e.target.value })} />
      <TextArea aria-label={`Category ${n}: how to recognise it`} rows={3}
        placeholder="How to recognise it, like: proof of a payment I made" value={category.description ?? ""}
        onChange={(e) => set(withDescription(category, e.target.value))} />
      <ListProblems messages={problems} />
    </>
  );
}

const KINDS: { value: FieldType; label: string }[] = [
  { value: "text", label: "Text" }, { value: "number", label: "Number" },
  { value: "date", label: "Date" }, { value: "yesNo", label: "Yes or no" },
];

/** Details a step hands back: a name, a kind, and what to look for. Used by Extract and Agent. */
function FieldsEditor({ fields, onChange, list, min }: { fields: Field[]; onChange: (fields: Field[]) => void; list: "fields" | "outputs"; min: number }) {
  const listProblems = useFieldProblems(list);
  return (
    <>
      <ListRows<Field> items={fields} noun="detail" addLabel="Add a detail" min={min}
        newItem={() => ({ name: "", type: "text" })} onChange={onChange}>
        {(f, i, set) => <FieldRow field={f} index={i} list={list} set={set} />}
      </ListRows>
      <ListProblems messages={listProblems} />
    </>
  );
}

function FieldRow({ field, index, list, set }: { field: Field; index: number; list: string; set: (f: Field) => void }) {
  const problems = useProblemsUnder(`${list}.${index}`);
  const n = index + 1;
  return (
    <>
      <div className={styles.pair}>
        <NameInput aria-label={`Detail ${n} name`} placeholder="Name, like vendor" value={field.name} onChange={(name) => set({ ...field, name })} />
        <Select aria-label={`Detail ${n} kind`} value={field.type} onChange={(e) => set({ ...field, type: e.target.value as FieldType })}>
          {KINDS.map((k) => <option key={k.value} value={k.value}>{k.label}</option>)}
        </Select>
      </div>
      <TextInput aria-label={`Detail ${n}: what to look for`} placeholder="What to look for, like: the total, including tax"
        value={field.description ?? ""} onChange={(e) => set(withDescription(field, e.target.value))} />
      {field.name && <p className={styles.hint}>Use it later as <code className={styles.chip}>{`{${field.name}}`}</code></p>}
      <ListProblems messages={problems} />
    </>
  );
}

export function ExtractForm({ step, onChange }: FormProps<"extract">) {
  return (
    <>
      <fieldset className={styles.section}>
        <legend className={styles.sectionTitle}>Details to pull out</legend>
        <FieldsEditor fields={step.fields} list="fields" min={1} onChange={(fields) => onChange({ ...step, fields })} />
      </fieldset>
      <div className={styles.field}>
        <span className={styles.label}>If a detail can't be found</span>
        <Segmented label="If a detail can't be found" value={step.ifMissing} onChange={(ifMissing) => onChange({ ...step, ifMissing })}
          options={[{ value: "review", label: "Ask me to check it" }, { value: "fail", label: "Stop the run" }]} />
        <p className={styles.hint}>Nothing is renamed, moved or logged with a missing detail.</p>
      </div>
    </>
  );
}

const STARTERS: { label: string; text: string }[] = [
  { label: "A one-paragraph summary", text: "Summarise this file in one paragraph, in plain words." },
  { label: "The action items", text: "List the action items in this file as bullet points, with who does each one and by when, if it says." },
  { label: "A short title", text: "Write a short title for this file, a few words long." },
  { label: "A polite reply", text: "Write a short, friendly reply to this message." },
];

export function WriteForm({ step, onChange }: FormProps<"write">) {
  const name = step.saveAs;
  return (
    <>
      <VariableText label="What should it write?" field="instruction" multiline value={step.instruction}
        onChange={(instruction) => onChange({ ...step, instruction })}
        hint="Say how long and in what tone, like: three bullet points, friendly." />
      {!step.instruction.trim() && (
        <div className={styles.starters} aria-label="Suggestions">
          {STARTERS.map((s) => (
            <Button key={s.label} variant="secondary" onClick={() => onChange({ ...step, instruction: s.text })}>{s.label}</Button>
          ))}
        </div>
      )}
      <FormField label="Save the text as" field="saveAs" hint={name ? <>Use it later as <code className={styles.chip}>{`{${name}}`}</code></> : undefined}>
        {({ id, describedBy }) => (
          <NameInput id={id} aria-describedby={describedBy} value={name} onChange={(saveAs) => onChange({ ...step, saveAs })} />
        )}
      </FormField>
    </>
  );
}

const ABILITIES: { id: string; label: string }[] = [{ id: "readFile", label: "Read the file's contents" }];

export function AgentForm({ step, onChange }: FormProps<"agent">) {
  const known = new Set(ABILITIES.map((a) => a.id));
  const toggle = (id: string, on: boolean) =>
    onChange({ ...step, abilities: on ? [...step.abilities.filter((a) => a !== id), id] : step.abilities.filter((a) => a !== id) });
  return (
    <>
      <VariableText label="What should it do?" field="instruction" multiline value={step.instruction}
        onChange={(instruction) => onChange({ ...step, instruction })}
        hint="Use this when no other step fits. For pulling out details, Extract is faster and checks the results." />
      <fieldset className={styles.section}>
        <legend className={styles.sectionTitle}>It may</legend>
        {ABILITIES.map((a) => (
          <label key={a.id} className={styles.check}>
            <input type="checkbox" checked={step.abilities.includes(a.id)} onChange={(e) => toggle(a.id, e.target.checked)} />
            {a.label}
          </label>
        ))}
        {step.abilities.filter((a) => !known.has(a)).map((a) => <p key={a} className={styles.hint}>Other: {a}</p>)}
      </fieldset>
      <fieldset className={styles.section}>
        <legend className={styles.sectionTitle}>What should it hand back?</legend>
        <FieldsEditor fields={step.outputs} list="outputs" min={0} onChange={(outputs) => onChange({ ...step, outputs })} />
      </fieldset>
    </>
  );
}
