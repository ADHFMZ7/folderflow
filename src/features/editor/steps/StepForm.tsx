// The settings form for a step, by its type. Each form reports a whole new
// step through onChange, and nothing else.

import type { Step } from "../../../api/types";
import { AddRowForm, CreateFileForm, MoveForm, NotifyForm, RenameForm, TagForm } from "./actions";
import { AgentForm, ClassifyForm, ExtractForm, WriteForm } from "./ai";
import { AskMeForm, IfForm, StopForm } from "./logic";
import { FileAddedForm, RunNowForm, ScheduleForm } from "./triggers";

export function StepForm({ step, onChange }: { step: Step; onChange: (step: Step) => void }) {
  const props = { onChange };
  switch (step.type) {
    case "fileAdded": return <FileAddedForm step={step} {...props} />;
    case "schedule": return <ScheduleForm step={step} {...props} />;
    case "runNow": return <RunNowForm />;
    case "classify": return <ClassifyForm step={step} {...props} />;
    case "extract": return <ExtractForm step={step} {...props} />;
    case "write": return <WriteForm step={step} {...props} />;
    case "agent": return <AgentForm step={step} {...props} />;
    case "rename": return <RenameForm step={step} {...props} />;
    case "move": return <MoveForm step={step} {...props} />;
    case "createFile": return <CreateFileForm step={step} {...props} />;
    case "tag": return <TagForm step={step} {...props} />;
    case "addRow": return <AddRowForm step={step} {...props} />;
    case "notify": return <NotifyForm step={step} {...props} />;
    case "if": return <IfForm step={step} {...props} />;
    case "stop": return <StopForm />;
    case "askMe": return <AskMeForm step={step} {...props} />;
  }
}

/** The fields each form shows problems for; problems about anything else go in the list at the top. */
export const SHOWN_FIELDS: { [T in Step["type"]]: string[] } = {
  fileAdded: ["folder", "fileTypes"],
  schedule: ["schedule"],
  runNow: [],
  classify: ["categories", "instructions"],
  extract: ["fields"],
  write: ["instruction", "saveAs"],
  agent: ["instruction", "outputs"],
  rename: ["template"],
  move: ["to"],
  createFile: ["name", "folder", "contents"],
  tag: ["tags"],
  addRow: ["file", "columns", "headers"],
  notify: ["message"],
  if: ["condition"],
  stop: [],
  askMe: ["question", "answers"],
};
