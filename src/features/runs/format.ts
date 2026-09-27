// How runs are worded on screen: when, what state, and what undo did.

import type { RunStatus, RunSummary, UndoReport } from "../../api/types";

/** "just now", "5 min ago", "3 h ago", "yesterday", "4 days ago", then the date. */
export function ago(iso: string, now = Date.now()): string {
  const seconds = Math.max(0, Math.round((now - Date.parse(iso)) / 1000));
  if (seconds < 60) return "just now";
  const minutes = Math.round(seconds / 60);
  if (minutes < 60) return `${minutes} min ago`;
  const hours = Math.round(minutes / 60);
  if (hours < 24) return `${hours} h ago`;
  const days = Math.round(hours / 24);
  if (days === 1) return "yesterday";
  if (days < 7) return `${days} days ago`;
  return new Date(iso).toLocaleDateString(undefined, { day: "numeric", month: "short", year: "numeric" });
}

export const STATUS_WORDS: Record<RunStatus, string> = {
  queued: "Queued", running: "Running", waiting: "Waiting for you", done: "Done",
  failed: "Failed", interrupted: "Stopped", undone: "Undone",
};

export const statusTone = (status: RunStatus): "ok" | "danger" | "neutral" =>
  status === "done" ? "ok" : status === "failed" || status === "interrupted" || status === "waiting" ? "danger" : "neutral";

/** The file a run was for, or the workflow's name for a run on no file. */
export const runName = (run: RunSummary) => run.file ?? run.workflowName;

const plural = (n: number, word: string) => `${n} ${word}${n === 1 ? "" : "s"}`;

/** "Put back 3 changes.", with a word on what was left alone. */
export function undoWords(report: UndoReport): string {
  const put = report.restored ? `Put back ${plural(report.restored, "change")}.` : "There was nothing to put back.";
  if (!report.leftAlone.length) return put;
  return `${put} ${plural(report.leftAlone.length, "file")} changed since, so ${report.leftAlone.length === 1 ? "it was" : "they were"} left alone.`;
}
