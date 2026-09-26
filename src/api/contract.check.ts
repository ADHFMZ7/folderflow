// Fails to compile if a type generated from Rust (src/api/generated) differs from the
// front end's own in types.ts, so the two sides of the api can't drift apart.
import type * as T from "./types";
import type { Settings as GSettings } from "./generated/Settings";
import type { Connection as GConnection } from "./generated/Connection";
import type { ConnectOutcome as GConnectOutcome } from "./generated/ConnectOutcome";
import type { DetectResult as GDetectResult } from "./generated/DetectResult";
import type { LoadedSettings as GLoadedSettings } from "./generated/LoadedSettings";
import type { Model as GModel } from "./generated/Model";
import type { ModelKind as GModelKind } from "./generated/ModelKind";
import type { Provider as GProvider } from "./generated/Provider";
import type { SettingsChange as GSettingsChange } from "./generated/SettingsChange";
import type { Credentials as GCredentials } from "./generated/Credentials";
import type { WorkflowSummary as GWorkflowSummary } from "./generated/WorkflowSummary";
import type { Template as GTemplate } from "./generated/Template";
import type { ErrorCode as GErrorCode } from "./generated/ErrorCode";
import type { Workflow as GWorkflow } from "./generated/Workflow";
import type { Step as GStep } from "./generated/Step";
import type { Branch as GBranch } from "./generated/Branch";
import type { Position as GPosition } from "./generated/Position";
import type { Schedule as GSchedule } from "./generated/Schedule";
import type { Field as GField } from "./generated/Field";
import type { Condition as GCondition } from "./generated/Condition";
import type { Problem as GProblem } from "./generated/Problem";
import type { ProblemCode as GProblemCode } from "./generated/ProblemCode";
import type { SaveResult as GSaveResult } from "./generated/SaveResult";

type Same<A, B> = [A] extends [B] ? ([B] extends [A] ? true : false) : false;
type Check<T extends true> = T;

export type ContractChecks = [
  Check<Same<GSettings, T.Settings>>,
  Check<Same<GConnection, T.Connection>>,
  Check<Same<GConnectOutcome, T.ConnectOutcome>>,
  Check<Same<GDetectResult, T.DetectResult>>,
  Check<Same<GLoadedSettings, T.LoadedSettings>>,
  Check<Same<GModel, T.Model>>,
  Check<Same<GModelKind, T.ModelKind>>,
  Check<Same<GProvider, T.Provider>>,
  Check<Same<GSettingsChange, T.SettingsChange>>,
  Check<Same<GCredentials, T.Credentials>>,
  Check<Same<GWorkflowSummary, T.WorkflowSummary>>,
  Check<Same<GTemplate, T.Template>>,
  Check<Same<GErrorCode, T.ApiErrorCode>>,
  Check<Same<GWorkflow, T.Workflow>>,
  Check<Same<GStep, T.Step>>,
  Check<Same<GBranch, T.Branch>>,
  Check<Same<GPosition, T.Position>>,
  Check<Same<GSchedule, T.Schedule>>,
  Check<Same<GField, T.Field>>,
  Check<Same<GCondition, T.Condition>>,
  Check<Same<GProblem, T.Problem>>,
  Check<Same<GProblemCode, T.ProblemCode>>,
  Check<Same<GSaveResult, T.SaveResult>>,
];
