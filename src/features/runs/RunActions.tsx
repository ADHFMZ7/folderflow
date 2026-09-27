// The choices a run that needs the person offers: the answers to its question,
// or Retry / Resume, Undo run and Dismiss. Shared by Needs you and the run page.

import { useState } from "react";
import { useApi } from "../../api/api";
import { ApiError, type NeedsYouItem } from "../../api/types";
import { Button } from "../../ui";
import { undoWords } from "./format";
import styles from "./runs.module.css";

type Props = {
  item: NeedsYouItem;
  /** Says how an action went: what undo put back, or why it was refused. */
  onResult: (message: string | null) => void;
};

export function RunActions({ item, onResult }: Props) {
  const api = useApi();
  const [busy, setBusy] = useState(false);
  const id = item.run.id;

  async function act(action: () => Promise<string | null>) {
    setBusy(true);
    try {
      onResult(await action());
    } catch (e) {
      onResult(e instanceof ApiError ? e.message : String(e));
    } finally {
      setBusy(false);
    }
  }

  if (item.kind === "question") {
    return (
      <div className={styles.actions} role="group" aria-label="Answers">
        {item.answers.map((a) => (
          <Button key={a.id} variant="secondary" disabled={busy} onClick={() => act(async () => { await api.answer(id, a.id); return null; })}>
            {a.label}
          </Button>
        ))}
      </div>
    );
  }
  return (
    <div className={styles.actions}>
      {item.kind === "failed"
        ? <Button disabled={busy} onClick={() => act(async () => { await api.retryRun(id); return null; })}
            title={item.step ? `Run again from ${item.step}` : undefined}>Retry</Button>
        : <Button disabled={busy} onClick={() => act(async () => { await api.resumeRun(id); return null; })}
            title="Carry on from where it stopped">Resume</Button>}
      <Button variant="secondary" disabled={busy} onClick={() => act(async () => undoWords(await api.undoRun(id)))}
        title="Put back every change this run made">Undo run</Button>
      <Button variant="quiet" disabled={busy} onClick={() => act(async () => { await api.dismissRun(id); return null; })}
        title="Leave it as it is and take it off this list">Dismiss</Button>
    </div>
  );
}
