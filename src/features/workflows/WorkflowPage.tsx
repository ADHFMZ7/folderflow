// One workflow. The Studio editor replaces this page's body in the next step;
// for now it shows what the workflow contains.

import { useEffect, useState } from "react";
import { useApi } from "../../api/api";
import { ApiError, type Workflow } from "../../api/types";
import { hrefFor } from "../../app/routes";
import { Card, Spinner } from "../../ui";
import styles from "./workflows.module.css";

export function WorkflowPage({ id }: { id: string }) {
  const api = useApi();
  const [workflow, setWorkflow] = useState<Workflow | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let live = true;
    api.getWorkflow(id).then(
      (wf) => { if (live) setWorkflow(wf); },
      (e) => { if (live) setError(e instanceof ApiError ? e.message : String(e)); },
    );
    return () => { live = false; };
  }, [api, id]);

  return (
    <div className={styles.page}>
      <a className={styles.back} href={hrefFor({ page: "workflows" })}>All workflows</a>
      {error ? <p className={styles.muted}>{error}</p>
        : !workflow ? <Spinner label="Opening workflow…" />
        : (
          <>
            <h1>{workflow.name}</h1>
            <Card className={styles.card}>
              <p className={styles.muted}>The editor for this workflow is being built. It has these steps:</p>
              <ol className={styles.steps}>{workflow.steps.map((s) => <li key={s.id}>{s.title}</li>)}</ol>
            </Card>
          </>
        )}
    </div>
  );
}
