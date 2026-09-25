import { useEffect, useState } from "react";
import { useApi } from "../../api/api";
import type { Template, WorkflowSummary } from "../../api/types";

/** Saved workflows and the template catalog; null until loaded. */
export function useWorkflows() {
  const api = useApi();
  const [workflows, setWorkflows] = useState<WorkflowSummary[] | null>(null);
  const [templates, setTemplates] = useState<Template[]>([]);

  useEffect(() => {
    let live = true;
    Promise.all([api.listWorkflows(), api.listTemplates()]).then(([w, t]) => {
      if (live) { setWorkflows(w); setTemplates(t); }
    });
    return () => { live = false; };
  }, [api]);

  return { workflows, templates };
}
