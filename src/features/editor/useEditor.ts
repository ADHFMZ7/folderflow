// The editor's state: the saved workflow, the draft being edited, and its
// problems. Edits only change the draft; Save writes it through the api.

import { useCallback, useEffect, useRef, useState } from "react";
import { useApi } from "../../api/api";
import { ApiError, type Problem, type Workflow } from "../../api/types";

const VALIDATE_DELAY_MS = 250;

export type EditorState =
  | { status: "loading" }
  | { status: "failed"; message: string }
  | {
    status: "ready";
    draft: Workflow;
    problems: Problem[];
    dirty: boolean;
    saving: boolean;
    /** Why the last save failed; `conflict` means the file changed elsewhere. */
    saveError: { conflict: boolean; message: string } | null;
  };

export function useEditor(id: string) {
  const api = useApi();
  const [state, setState] = useState<EditorState>({ status: "loading" });
  /** The draft the current problems belong to. */
  const validated = useRef<Workflow | null>(null);

  const load = useCallback(async () => {
    try {
      const workflow = await api.getWorkflow(id);
      const problems = await api.validateWorkflow(workflow);
      validated.current = workflow;
      setState({ status: "ready", draft: workflow, problems, dirty: false, saving: false, saveError: null });
    } catch (e) {
      setState({ status: "failed", message: e instanceof Error ? e.message : String(e) });
    }
  }, [api, id]);

  useEffect(() => { load(); }, [load]);

  // Re-validate once edits pause. This lives in an effect, not in `edit`: state
  // updaters must stay pure, and React may run them twice.
  const draft = state.status === "ready" ? state.draft : null;
  useEffect(() => {
    if (!draft || draft === validated.current) return;
    const timer = window.setTimeout(async () => {
      const problems = await api.validateWorkflow(draft);
      validated.current = draft;
      setState((cur) => (cur.status === "ready" && cur.draft === draft ? { ...cur, problems } : cur));
    }, VALIDATE_DELAY_MS);
    return () => window.clearTimeout(timer);
  }, [api, draft]);

  /** Applies an edit to the draft. */
  const edit = useCallback((change: (w: Workflow) => Workflow) => {
    setState((s) => {
      if (s.status !== "ready") return s;
      const next = change(s.draft);
      return next === s.draft ? s : { ...s, draft: next, dirty: true };
    });
  }, []);

  /** Saves the draft. Resolves to whether everything is now saved. */
  const save = useCallback(async (): Promise<boolean> => {
    if (state.status !== "ready") return false;
    if (!state.dirty) return true;
    if (state.saving) return false;
    const sent = state.draft;
    setState({ ...state, saving: true, saveError: null });
    try {
      const { workflow, problems } = await api.saveWorkflow(sent);
      validated.current = workflow;
      // Keep edits made while the save was in flight; they're still unsaved.
      setState((cur) => cur.status !== "ready" ? cur
        : cur.draft === sent ? { ...cur, draft: workflow, problems, dirty: false, saving: false }
        : { ...cur, draft: { ...cur.draft, revision: workflow.revision }, saving: false });
      return true;
    } catch (e) {
      const conflict = e instanceof ApiError && e.code === "conflict";
      const message = e instanceof Error ? e.message : String(e);
      setState((cur) => (cur.status === "ready" ? { ...cur, saving: false, saveError: { conflict, message } } : cur));
      return false;
    }
  }, [api, state]);

  return { state, edit, save, reload: load };
}
