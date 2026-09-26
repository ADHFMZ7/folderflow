// Settings for the triggers: File added, Schedule and Run now.

import { useState } from "react";
import type { Schedule, Step } from "../../../api/types";
import { Segmented, Select, TextInput, Toggle } from "../../../ui";
import { useProblemsUnder } from "./context";
import { FolderField, FormField, ListProblems, Note } from "./controls";
import styles from "./Steps.module.css";

export type StepOf<T extends Step["type"]> = Extract<Step, { type: T }>;
export type FormProps<T extends Step["type"]> = { step: StepOf<T>; onChange: (step: StepOf<T>) => void };

const PRESETS: { label: string; types: string[] }[] = [
  { label: "Any file", types: [] },
  { label: "PDFs", types: ["pdf"] },
  { label: "Images", types: ["png", "jpg", "jpeg", "heic", "gif", "webp"] },
  { label: "Documents", types: ["pdf", "doc", "docx", "pages", "txt", "rtf", "md"] },
  { label: "Spreadsheets", types: ["csv", "xls", "xlsx", "numbers"] },
];

const same = (a: string[], b: string[]) => a.length === b.length && a.every((x) => b.includes(x));

/** ".PDF " → "pdf". */
const toExtension = (text: string) => text.trim().replace(/^\.+/, "").toLowerCase();

function FileTypes({ value, onChange }: { value: string[]; onChange: (types: string[]) => void }) {
  const [typed, setTyped] = useState("");
  const problems = useProblemsUnder("fileTypes");
  const add = () => {
    const ext = toExtension(typed);
    if (ext && !value.includes(ext)) onChange([...value, ext]);
    setTyped("");
  };
  return (
    <fieldset className={styles.section}>
      <legend className={styles.label}>Which files</legend>
      <div className={styles.presets}>
        {PRESETS.map((p) => (
          <button key={p.label} type="button" className={styles.preset} aria-pressed={same(value, p.types)} onClick={() => onChange(p.types)}>
            {p.label}
          </button>
        ))}
      </div>
      {value.length > 0 && (
        <div className={styles.chips}>
          {value.map((t) => (
            <span key={t} className={styles.typeChip}>
              {t}
              <button type="button" aria-label={`Remove ${t}`} onClick={() => onChange(value.filter((x) => x !== t))}>✕</button>
            </span>
          ))}
        </div>
      )}
      <TextInput aria-label="Add a file type" placeholder="Add a type, like pdf" value={typed}
        onChange={(e) => setTyped(e.target.value)} onBlur={add}
        onKeyDown={(e) => { if (e.key === "Enter" || e.key === ",") { e.preventDefault(); add(); } }} />
      <ListProblems messages={problems} />
      <p className={styles.hint}>{value.length ? "Only files ending in these run the workflow." : "Every new file runs the workflow."}</p>
    </fieldset>
  );
}

export function FileAddedForm({ step, onChange }: FormProps<"fileAdded">) {
  return (
    <>
      <FolderField label="Folder to watch" field="folder" value={step.folder} onChange={(folder) => onChange({ ...step, folder })}
        hint="Runs once for each new file here. Files FolderFlow makes itself never start it." />
      <FileTypes value={step.fileTypes} onChange={(fileTypes) => onChange({ ...step, fileTypes })} />
      <Toggle label="Include files in subfolders" checked={step.subfolders} onChange={(subfolders) => onChange({ ...step, subfolders })} />
      <Note>The next steps can use the file's name, extension, folder, the date it was added, and the year.</Note>
    </>
  );
}

const DAYS = ["Sunday", "Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday"];

export function ScheduleForm({ step, onChange }: FormProps<"schedule">) {
  const { schedule } = step;
  const setSchedule = (next: Schedule) => onChange({ ...step, schedule: next });
  const setEvery = (every: Schedule["every"]) => {
    const { weekday, ...rest } = schedule;
    setSchedule(every === "week" ? { ...rest, every, weekday: weekday ?? 5 } : { ...rest, every });
  };
  return (
    <>
      <div className={styles.field}>
        <span className={styles.label}>How often</span>
        <Segmented label="How often" value={schedule.every} onChange={setEvery}
          options={[{ value: "day", label: "Every day" }, { value: "weekday", label: "Weekdays" }, { value: "week", label: "Once a week" }]} />
      </div>
      {schedule.every === "week" && (
        <FormField label="Day" field="schedule.weekday">
          {({ id, describedBy }) => (
            <Select id={id} aria-describedby={describedBy} value={String(schedule.weekday ?? 5)}
              onChange={(e) => setSchedule({ ...schedule, weekday: Number(e.target.value) })}>
              {DAYS.map((d, i) => <option key={d} value={i}>{d}</option>)}
            </Select>
          )}
        </FormField>
      )}
      <FormField label="Time" field="schedule.time">
        {({ id, describedBy }) => (
          <TextInput id={id} type="time" aria-describedby={describedBy} value={schedule.time}
            onChange={(e) => setSchedule({ ...schedule, time: e.target.value })} />
        )}
      </FormField>
      <Note>The next steps can use today's date and the year. There's no file in a scheduled run.</Note>
    </>
  );
}

export function RunNowForm() {
  return <Note>Runs when you pick files and choose Run. The next steps get the same details as File added.</Note>;
}
