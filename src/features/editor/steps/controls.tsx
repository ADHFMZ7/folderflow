// The controls step forms are built from: labelled fields that show their own
// problems, text that takes {details}, pickers, names, previews and lists.

import {
  useEffect, useId, useLayoutEffect, useRef, useState,
  type ChangeEvent, type KeyboardEvent, type ReactNode,
} from "react";
import { useApi } from "../../../api/api";
import { Button, TextArea, TextInput } from "../../../ui";
import { useFieldProblems, useStepContext } from "./context";
import { unknownIn, type Available } from "./variables";
import styles from "./Steps.module.css";

type Described = { hint?: ReactNode; problems?: string[]; preview?: string | null };

/** The ids and nodes that describe a control: its problems, preview and hint. */
function useDescription({ hint, problems = [], preview }: Described) {
  const base = useId();
  const parts: { id: string; node: ReactNode; className: string }[] = [];
  if (problems.length) parts.push({ id: `${base}-p`, className: styles.problem, node: problems.map((m) => <span key={m}>{m} </span>) });
  if (preview) parts.push({ id: `${base}-x`, className: styles.preview, node: `Example: ${preview}` });
  if (hint) parts.push({ id: `${base}-h`, className: styles.hint, node: hint });
  return {
    describedBy: parts.length ? parts.map((p) => p.id).join(" ") : undefined,
    nodes: parts.map((p) => <p key={p.id} id={p.id} className={p.className}>{p.node}</p>),
  };
}

/** A label above a control, and what describes it below. The control gets `id` and `describedBy`. */
export function FormField({ label, field, hint, preview, extraProblems = [], children }: {
  label: string;
  /** The field's path in the step, for its problems. */
  field?: string;
  hint?: ReactNode;
  preview?: string | null;
  extraProblems?: string[];
  children: (control: { id: string; describedBy?: string }) => ReactNode;
}) {
  const id = useId();
  const problems = [...extraProblems, ...useFieldProblems(field)];
  const { describedBy, nodes } = useDescription({ hint, problems, preview });
  return (
    <div className={styles.field}>
      <label className={styles.label} htmlFor={id}>{label}</label>
      {children({ id, describedBy })}
      {nodes}
    </div>
  );
}

/** Problems about a whole list or row, shown where it is. */
export function ListProblems({ messages }: { messages: string[] }) {
  if (!messages.length) return null;
  return <p className={styles.problem}>{messages.join(" ")}</p>;
}

type Menu = { mode: "insert" } | { mode: "suggest"; start: number; query: string };

const matching = (available: Available[], query: string) => {
  const q = query.toLowerCase();
  return available.filter((v) => v.name.toLowerCase().startsWith(q) || v.label.toLowerCase().startsWith(q));
};

/** Groups by source, keeping the nearest-first order. */
function bySource(available: Available[]) {
  const groups = new Map<string, Available[]>();
  for (const v of available) groups.set(v.source, [...(groups.get(v.source) ?? []), v]);
  return [...groups];
}

/**
 * Text that can use {details}: an Insert list grouped by where each comes from,
 * suggestions after typing "{", and a warning for details nothing before the step gives.
 */
export function VariableText({ label, value, onChange, field, multiline, hint, preview, placeholder, after }: {
  label: string;
  value: string;
  onChange: (value: string) => void;
  field: string;
  multiline?: boolean;
  hint?: ReactNode;
  preview?: string | null;
  placeholder?: string;
  /** Extra controls beside the Insert button, e.g. a picker. */
  after?: ReactNode;
}) {
  const { available, reachable } = useStepContext();
  const id = useId();
  const listId = `${id}-list`;
  const input = useRef<HTMLInputElement & HTMLTextAreaElement>(null);
  const wrapper = useRef<HTMLDivElement>(null);
  /** Where the selection was last: an insert replaces it. */
  const caret = useRef<[number, number] | null>(null);
  const pendingCaret = useRef<number | null>(null);
  const [menu, setMenu] = useState<Menu | null>(null);
  const [active, setActive] = useState(0);

  const unknown = unknownIn(value, available).map((n) => `Nothing before this step gives {${n}}.`);
  const problems = [...unknown, ...useFieldProblems(field, { skipUnknown: true })];
  const { describedBy, nodes } = useDescription({ hint, problems, preview });

  const options = menu?.mode === "suggest" ? matching(available, menu.query) : available;
  const open = menu !== null && (menu.mode === "insert" || options.length > 0);

  useLayoutEffect(() => {
    if (pendingCaret.current === null || !input.current) return;
    input.current.focus();
    input.current.setSelectionRange(pendingCaret.current, pendingCaret.current);
    pendingCaret.current = null;
  });

  useEffect(() => {
    if (!menu) return;
    const onDown = (e: MouseEvent) => { if (!wrapper.current?.contains(e.target as Node)) setMenu(null); };
    document.addEventListener("mousedown", onDown);
    return () => document.removeEventListener("mousedown", onDown);
  }, [menu]);

  const remember = () => {
    const el = input.current;
    if (el) caret.current = [el.selectionStart ?? el.value.length, el.selectionEnd ?? el.value.length];
  };

  const pick = (v: Available) => {
    const token = `{${v.name}}`;
    const [start, end] = caret.current ?? [value.length, value.length];
    const [from, to] = menu?.mode === "suggest" ? [menu.start, start] : [start, end];
    pendingCaret.current = from + token.length;
    onChange(value.slice(0, from) + token + value.slice(to));
    setMenu(null);
  };

  const onText = (e: ChangeEvent<HTMLInputElement | HTMLTextAreaElement>) => {
    const text = e.target.value;
    const at = e.target.selectionStart ?? text.length;
    caret.current = [at, at];
    const open = /\{([A-Za-z0-9_]*)$/.exec(text.slice(0, at));
    if (open) {
      setMenu({ mode: "suggest", start: at - open[0].length, query: open[1] });
      setActive(0);
    } else if (menu?.mode === "suggest") {
      setMenu(null);
    }
    onChange(text);
  };

  const onKey = (e: KeyboardEvent) => {
    if (!open) return;
    if (e.key === "Escape") { e.preventDefault(); setMenu(null); return; }
    if (!options.length) return;
    if (e.key === "ArrowDown") { e.preventDefault(); setActive((a) => (a + 1) % options.length); }
    if (e.key === "ArrowUp") { e.preventDefault(); setActive((a) => (a - 1 + options.length) % options.length); }
    if (e.key === "Enter") { e.preventDefault(); pick(options[Math.min(active, options.length - 1)]); }
  };

  const optionId = (i: number) => `${id}-o${i}`;
  const common = {
    id, ref: input, value, placeholder,
    onChange: onText, onKeyDown: onKey, onSelect: remember, onKeyUp: remember, onClick: remember,
    "aria-describedby": describedBy,
    "aria-autocomplete": "list" as const,
    "aria-controls": open ? listId : undefined,
    "aria-activedescendant": open && menu?.mode === "suggest" && options.length ? optionId(Math.min(active, options.length - 1)) : undefined,
  };

  let index = -1;
  return (
    <div className={styles.field} ref={wrapper}>
      <div className={styles.labelRow}>
        <label className={styles.label} htmlFor={id}>{label}</label>
        <button type="button" className={styles.insert} aria-label={`Insert a detail into ${label}`} aria-expanded={open}
          onMouseDown={(e) => e.preventDefault()}
          onClick={() => setMenu((m) => (m?.mode === "insert" ? null : { mode: "insert" }))}>
          Insert {"{…}"}
        </button>
      </div>
      <div className={styles.inputRow}>
        {multiline ? <TextArea {...common} /> : <TextInput {...common} />}
        {after}
      </div>
      {open && (
        <div role="listbox" id={listId} aria-label="Details you can use" className={styles.menu}>
          {options.length === 0 && (
            <p className={styles.hint}>
              {reachable ? "There are no details to use here." : "Connect this step to use details from earlier steps."}
            </p>
          )}
          {bySource(options).map(([source, items]) => (
            <div role="group" aria-label={source} key={source} className={styles.menuGroup}>
              <div className={styles.menuSource} aria-hidden>{source}</div>
              {items.map((v) => {
                index += 1;
                const i = index;
                return (
                  <div key={v.name} role="option" id={optionId(i)} aria-label={v.label}
                    aria-selected={menu?.mode === "suggest" && i === Math.min(active, options.length - 1)}
                    className={styles.option} onMouseDown={(e) => e.preventDefault()} onClick={() => pick(v)}>
                    <span>{v.label}</span><code className={styles.chip}>{`{${v.name}}`}</code>
                  </div>
                );
              })}
            </div>
          ))}
        </div>
      )}
      {nodes}
    </div>
  );
}

/** A VariableText with a Choose… button that opens the native folder picker. */
export function FolderField(props: Omit<Parameters<typeof VariableText>[0], "after">) {
  const api = useApi();
  const choose = async () => {
    const path = await api.chooseFolder(props.value || undefined);
    if (path) props.onChange(path);
  };
  return <VariableText {...props}
    after={<Button variant="secondary" aria-label={`Choose a folder for ${props.label}`} onClick={choose}>Choose…</Button>} />;
}

/** A VariableText with a Choose… button for an existing .csv file. */
export function CsvField(props: Omit<Parameters<typeof VariableText>[0], "after">) {
  const api = useApi();
  const choose = async () => {
    const path = await api.chooseCsv(props.value || undefined);
    if (path) props.onChange(path);
  };
  return <VariableText {...props}
    after={<Button variant="secondary" aria-label={`Choose a spreadsheet for ${props.label}`} onClick={choose}>Choose…</Button>} />;
}

/** What people type, made into a detail name: letters, digits and _, up to 32. */
export const toName = (text: string) => text.replace(/\s/g, "_").replace(/[^A-Za-z0-9_]/g, "").slice(0, 32);

/** A text box for a detail's name that only takes what a name can hold. */
export function NameInput({ value, onChange, ...rest }: { value: string; onChange: (name: string) => void } & Omit<Parameters<typeof TextInput>[0], "value" | "onChange">) {
  return <TextInput {...rest} value={value} spellCheck={false} autoCapitalize="off" onChange={(e) => onChange(toName(e.target.value))} />;
}

/**
 * An editable list: remove, move up and down, and add. Buttons are named by
 * `noun` and position ("Remove category 2"), and remove stops at `min`.
 */
export function ListRows<T>({ items, onChange, noun, addLabel, min = 0, newItem, keyOf, children }: {
  items: T[];
  onChange: (items: T[]) => void;
  noun: string;
  addLabel: string;
  min?: number;
  newItem: () => T;
  keyOf?: (item: T, i: number) => string;
  children: (item: T, i: number, set: (item: T) => void) => ReactNode;
}) {
  const set = (i: number) => (item: T) => onChange(items.map((x, j) => (j === i ? item : x)));
  const swap = (i: number, j: number) => {
    const next = [...items];
    [next[i], next[j]] = [next[j], next[i]];
    onChange(next);
  };
  return (
    <div className={styles.list}>
      {items.map((item, i) => (
        <div key={keyOf?.(item, i) ?? i} className={styles.row}>
          <div className={styles.rowBody}>{children(item, i, set(i))}</div>
          <div className={styles.rowTools}>
            <button type="button" className={styles.tool} aria-label={`Move ${noun} ${i + 1} up`} disabled={i === 0} onClick={() => swap(i, i - 1)}>↑</button>
            <button type="button" className={styles.tool} aria-label={`Move ${noun} ${i + 1} down`} disabled={i === items.length - 1} onClick={() => swap(i, i + 1)}>↓</button>
            <button type="button" className={styles.tool} aria-label={`Remove ${noun} ${i + 1}`} disabled={items.length <= min}
              onClick={() => onChange(items.filter((_, j) => j !== i))}>✕</button>
          </div>
        </div>
      ))}
      <Button variant="quiet" className={styles.add} onClick={() => onChange([...items, newItem()])}><span aria-hidden>+</span>{addLabel}</Button>
    </div>
  );
}

/** A short explanation for steps with little or nothing to set. */
export function Note({ children }: { children: ReactNode }) {
  return <p className={styles.note}>{children}</p>;
}
