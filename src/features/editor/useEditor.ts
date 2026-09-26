// The editor's state. Edits change the draft at once and are saved on their own
// once they pause: straight to the workflow when it's off, or to its draft when
// it's on, so a half-finished change never runs until it's applied. Undo and
// redo step through snapshots of the draft. See "Drafts" in docs/workflow-format.md.

import { useCallback, useEffect, useRef, useState } from "react";
import { useApi } from "../../api/api";
import { ApiError, type Problem, type Workflow } from "../../api/types";

const SAVE_DELAY_MS = 500;
const VALIDATE_DELAY_MS = 250;
/** Edits in the same group this close together undo as one step. */
const GROUP_WINDOW_MS = 1000;

export type SaveStatus =
  | { kind: "saved" }
  | { kind: "pending" }
  | { kind: "saving" }
  | { kind: "failed"; conflict: boolean; message: string };

type Ready = {
  status: "ready";
  draft: Workflow;
  problems: Problem[];
  save: SaveStatus;
  /** A workflow that is on has changes in its draft that aren't live yet. */
  pendingApply: boolean;
  past: Workflow[];
  future: Workflow[];
};

export type EditorState = { status: "loading" } | { status: "failed"; message: string } | Ready;

const messageOf = (e: unknown) => (e instanceof Error ? e.message : String(e));

export function useEditor(id: string) {
  const api = useApi();
  const [state, setState] = useState<EditorState>({ status: "loading" });
  /** The workflow as it runs; its revision is the one every save is based on. */
  const live = useRef<Workflow | null>(null);
  /** The draft the current problems belong to. */
  const validated = useRef<Workflow | null>(null);
  /** The draft as last saved, so an unchanged draft isn't saved again. */
  const persisted = useRef<Workflow | null>(null);
  const lastGroup = useRef<{ key: string; at: number } | null>(null);
  const inFlight = useRef<Promise<boolean> | null>(null);

  const load = useCallback(async () => {
    try {
      const workflow = await api.getWorkflow(id);
      const waiting = workflow.enabled ? await api.getDraft(id) : null;
      const draft = waiting ?? workflow;
      const problems = await api.validateWorkflow(draft);
      live.current = workflow;
      validated.current = draft;
      persisted.current = draft;
      lastGroup.current = null;
      setState({ status: "ready", draft, problems, save: { kind: "saved" }, pendingApply: !!waiting, past: [], future: [] });
    } catch (e) {
      setState({ status: "failed", message: messageOf(e) });
    }
  }, [api, id]);

  useEffect(() => { load(); }, [load]);

  const draft = state.status === "ready" ? state.draft : null;
  const saveKind = state.status === "ready" ? state.save.kind : null;
  /** The draft as of the last render, for saves that run later. */
  const latestDraft = useRef<Workflow | null>(null);
  latestDraft.current = draft;

  // Re-validate once edits pause. Effects, not state updaters, do the work:
  // updaters must stay pure, and React may run them twice.
  useEffect(() => {
    if (!draft || draft === validated.current) return;
    const timer = window.setTimeout(async () => {
      const problems = await api.validateWorkflow(draft);
      validated.current = draft;
      setState((cur) => (cur.status === "ready" && cur.draft === draft ? { ...cur, problems } : cur));
    }, VALIDATE_DELAY_MS);
    return () => window.clearTimeout(timer);
  }, [api, draft]);

  /** Saves the current draft now. Resolves to whether everything is saved. */
  const flush = useCallback(async (): Promise<boolean> => {
    if (inFlight.current) await inFlight.current;
    const current = latestDraft.current;
    if (!current || !live.current) return false;
    if (current === persisted.current) return true;
    const base = live.current;
    const run = (async () => {
      setState((cur) => (cur.status === "ready" ? { ...cur, save: { kind: "saving" } } : cur));
      try {
        // The revision always comes from the running workflow, never from an
        // undo snapshot, so undoing past a save can't look like a conflict.
        const sent = { ...current, revision: base.revision };
        if (base.enabled) {
          await api.saveDraft(sent);
          persisted.current = current;
          setState((cur) => (cur.status === "ready"
            ? { ...cur, pendingApply: true, save: cur.draft === current ? { kind: "saved" } : { kind: "pending" } }
            : cur));
        } else {
          const { workflow } = await api.saveWorkflow(sent);
          live.current = workflow;
          persisted.current = current;
          setState((cur) => (cur.status === "ready"
            ? { ...cur, save: cur.draft === current ? { kind: "saved" } : { kind: "pending" } }
            : cur));
        }
        return true;
      } catch (e) {
        const conflict = e instanceof ApiError && e.code === "conflict";
        setState((cur) => (cur.status === "ready" ? { ...cur, save: { kind: "failed", conflict, message: messageOf(e) } } : cur));
        return false;
      }
    })();
    inFlight.current = run;
    try { return await run; } finally { if (inFlight.current === run) inFlight.current = null; }
  }, [api]);

  // Save once edits pause.
  useEffect(() => {
    if (saveKind !== "pending") return;
    const timer = window.setTimeout(() => { flush(); }, SAVE_DELAY_MS);
    return () => window.clearTimeout(timer);
  }, [draft, saveKind, flush]);

  /**
   * Applies an edit to the draft. Edits with the same `group` (a field being
   * typed into, a step being dragged) close together undo as one step.
   */
  const edit = useCallback((change: (w: Workflow) => Workflow, group?: string) => {
    const now = Date.now();
    const joins = !!group && lastGroup.current?.key === group && now - lastGroup.current.at < GROUP_WINDOW_MS;
    lastGroup.current = group ? { key: group, at: now } : null;
    setState((s) => {
      if (s.status !== "ready") return s;
      const next = change(s.draft);
      if (next === s.draft) return s;
      const past = joins ? s.past : [...s.past, s.draft];
      return { ...s, draft: next, past, future: [], save: { kind: "pending" } };
    });
  }, []);

  const undo = useCallback(() => {
    lastGroup.current = null;
    setState((s) => {
      if (s.status !== "ready" || !s.past.length) return s;
      const previous = s.past[s.past.length - 1];
      return { ...s, draft: previous, past: s.past.slice(0, -1), future: [s.draft, ...s.future], save: { kind: "pending" } };
    });
  }, []);

  const redo = useCallback(() => {
    lastGroup.current = null;
    setState((s) => {
      if (s.status !== "ready" || !s.future.length) return s;
      const [next, ...rest] = s.future;
      return { ...s, draft: next, past: [...s.past, s.draft], future: rest, save: { kind: "pending" } };
    });
  }, []);

  /** Makes the draft of a running workflow live. */
  const apply = useCallback(async () => {
    if (!(await flush())) return;
    try {
      const { workflow, problems } = await api.applyDraft(id);
      live.current = workflow;
      persisted.current = workflow;
      validated.current = workflow;
      setState((cur) => (cur.status === "ready" ? { ...cur, draft: workflow, problems, pendingApply: false, save: { kind: "saved" } } : cur));
    } catch (e) {
      const conflict = e instanceof ApiError && e.code === "conflict";
      setState((cur) => (cur.status === "ready" ? { ...cur, save: { kind: "failed", conflict, message: messageOf(e) } } : cur));
    }
  }, [api, flush, id]);

  /** Throws the draft of a running workflow away; the edit itself can still be undone. */
  const discard = useCallback(async () => {
    if (inFlight.current) await inFlight.current;
    try {
      await api.discardDraft(id);
      const running = live.current!;
      persisted.current = running;
      lastGroup.current = null;
      setState((cur) => (cur.status === "ready"
        ? { ...cur, draft: running, past: [...cur.past, cur.draft], future: [], pendingApply: false, save: { kind: "saved" } }
        : cur));
    } catch (e) {
      setState((cur) => (cur.status === "ready" ? { ...cur, save: { kind: "failed", conflict: false, message: messageOf(e) } } : cur));
    }
  }, [api, id]);

  return { state, edit, undo, redo, flush, apply, discard, reload: load };
}
