import { useCallback, useEffect, useState } from "react";
import { useApi } from "../../api/api";
import type { Template, WorkflowSummary } from "../../api/types";
import { navigate } from "../../app/routes";

/** Saved workflows and the template catalog, with the actions the home page offers. */
export function useWorkflows() {
  const api = useApi();
  const [workflows, setWorkflows] = useState<WorkflowSummary[] | null>(null);
  const [templates, setTemplates] = useState<Template[]>([]);

  const refresh = useCallback(async () => setWorkflows(await api.listWorkflows()), [api]);

  useEffect(() => {
    let live = true;
    Promise.all([api.listWorkflows(), api.listTemplates()]).then(([w, t]) => {
      if (live) { setWorkflows(w); setTemplates(t); }
    });
    return () => { live = false; };
  }, [api]);

  /** Creates a workflow (blank, or from a template) and opens it. */
  const create = useCallback(async (templateId: string | null) => {
    const wf = await api.createWorkflow(templateId);
    await refresh();
    navigate({ page: "workflow", id: wf.id });
  }, [api, refresh]);

  const remove = useCallback(async (id: string) => {
    await api.deleteWorkflow(id);
    await refresh();
  }, [api, refresh]);

  return { workflows, templates, create, remove, refresh };
}
