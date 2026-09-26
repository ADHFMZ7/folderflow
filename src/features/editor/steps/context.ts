// What every control in a step's form needs to know: the details it can offer,
// and the step's problems so each field can show its own.

import { createContext, useContext } from "react";
import type { Problem } from "../../../api/types";
import type { Available } from "./variables";

export type StepContextValue = {
  available: Available[];
  /** False for a step no path from the trigger reaches: it has no details to offer. */
  reachable: boolean;
  /** This step's problems. */
  problems: Problem[];
};

export const StepContext = createContext<StepContextValue>({ available: [], reachable: true, problems: [] });

export const useStepContext = () => useContext(StepContext);

/** Messages of the problems about exactly `field`. */
export function useFieldProblems(field: string | undefined, options: { skipUnknown?: boolean } = {}): string[] {
  const { problems } = useStepContext();
  if (!field) return [];
  return problems
    .filter((p) => p.field === field && !(options.skipUnknown && p.code === "unknown_variable"))
    .map((p) => p.message);
}

/** Messages of the problems about `path` or anything inside it, e.g. a list row. */
export function useProblemsUnder(path: string): string[] {
  const { problems } = useStepContext();
  return problems.filter((p) => p.field && isUnder(p.field, path)).map((p) => p.message);
}

export const isUnder = (field: string, path: string) => field === path || field.startsWith(`${path}.`);
