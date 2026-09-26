// Settings for the actions: Rename, Move / Copy, Create file, Tag, Add row and Notify.

import { Segmented, TextInput } from "../../../ui";
import { useFieldProblems, useStepContext } from "./context";
import { CsvField, FolderField, ListProblems, ListRows, VariableText } from "./controls";
import type { FormProps } from "./triggers";
import { fillSample } from "./variables";
import styles from "./Steps.module.css";

/** A preview of `text` with sample values, or nothing while it's empty. */
function useSample() {
  const { available } = useStepContext();
  return (text: string) => (text.trim() ? fillSample(text, available) : null);
}

export function RenameForm({ step, onChange }: FormProps<"rename">) {
  const { available } = useStepContext();
  const sample = useSample();
  const extension = available.find((v) => v.name === "extension")?.sample;
  const name = sample(step.template);
  return (
    <VariableText label="New name" field="template" value={step.template} onChange={(template) => onChange({ ...step, template })}
      preview={name && (extension ? `${name}.${extension}` : name)}
      hint="The extension stays the same. If the name is taken, a number is added so nothing is overwritten." />
  );
}

export function MoveForm({ step, onChange }: FormProps<"move">) {
  const sample = useSample();
  return (
    <>
      <div className={styles.field}>
        <span className={styles.label}>Move or copy</span>
        <Segmented label="Move or copy" value={step.mode} onChange={(mode) => onChange({ ...step, mode })}
          options={[{ value: "move", label: "Move" }, { value: "copy", label: "Copy" }]} />
      </div>
      <FolderField label="To folder" field="to" value={step.to} onChange={(to) => onChange({ ...step, to })}
        preview={sample(step.to)} hint="Folders that don't exist yet are made. Details like {year} make a folder per value." />
    </>
  );
}

export function CreateFileForm({ step, onChange }: FormProps<"createFile">) {
  const sample = useSample();
  return (
    <>
      <VariableText label="File name" field="name" value={step.name} onChange={(name) => onChange({ ...step, name })}
        preview={sample(step.name)} hint="Include the extension, like .txt or .md." />
      <FolderField label="In folder" field="folder" value={step.folder ?? ""} placeholder="The file's folder"
        onChange={(folder) => {
          const { folder: _, ...rest } = step;
          onChange(folder ? { ...rest, folder } : rest);
        }} />
      <VariableText label="Contents" field="contents" multiline value={step.contents} onChange={(contents) => onChange({ ...step, contents })} />
    </>
  );
}

export function TagForm({ step, onChange }: FormProps<"tag">) {
  const problems = useFieldProblems("tags");
  return (
    <fieldset className={styles.section}>
      <legend className={styles.sectionTitle}>Finder tags</legend>
      <ListRows<string> items={step.tags} noun="tag" addLabel="Add a tag" min={1} newItem={() => ""}
        onChange={(tags) => onChange({ ...step, tags })}>
        {(tag, i, set) => <VariableText label={`Tag ${i + 1}`} field={`tags.${i}`} value={tag} onChange={set} />}
      </ListRows>
      <ListProblems messages={problems} />
      <p className={styles.hint}>A detail like {"{category}"} tags each file with its own value.</p>
    </fieldset>
  );
}

type Column = { heading: string; value: string };

export function AddRowForm({ step, onChange }: FormProps<"addRow">) {
  const sample = useSample();
  const problems = [...useFieldProblems("columns"), ...useFieldProblems("headers")];
  const columns: Column[] = step.columns.map((value, i) => ({ value, heading: step.headers?.[i] ?? "" }));
  const setColumns = (next: Column[]) => {
    const { headers: _, ...rest } = step;
    const keepHeadings = step.headers !== undefined || next.some((c) => c.heading);
    onChange({ ...rest, columns: next.map((c) => c.value), ...(keepHeadings ? { headers: next.map((c) => c.heading) } : {}) });
  };
  const row = step.columns.some((c) => c.trim()) ? step.columns.map((c) => sample(c) ?? "").join(", ") : null;
  return (
    <>
      <CsvField label="Spreadsheet" field="file" value={step.file} onChange={(file) => onChange({ ...step, file })}
        placeholder="~/Documents/Log.csv" hint="Choose a .csv file, or type the path for a new one." />
      <fieldset className={styles.section}>
        <legend className={styles.sectionTitle}>Columns</legend>
        <ListRows<Column> items={columns} noun="column" addLabel="Add a column" min={1} newItem={() => ({ heading: "", value: "" })}
          onChange={setColumns}>
          {(c, i, set) => (
            <>
              <TextInput aria-label={`Column ${i + 1} heading`} placeholder="Heading, like Amount" value={c.heading}
                onChange={(e) => set({ ...c, heading: e.target.value })} />
              <VariableText label={`Column ${i + 1} value`} field={`columns.${i}`} value={c.value} onChange={(value) => set({ ...c, value })} />
            </>
          )}
        </ListRows>
        <ListProblems messages={problems} />
        {row && <p className={styles.preview}>Example row: {row}</p>}
        <p className={styles.hint}>A new file starts with a row of the headings.</p>
      </fieldset>
    </>
  );
}

export function NotifyForm({ step, onChange }: FormProps<"notify">) {
  const sample = useSample();
  return (
    <VariableText label="Message" field="message" value={step.message} onChange={(message) => onChange({ ...step, message })}
      preview={sample(step.message)} hint="Shown as a macOS notification." />
  );
}
