// Reading which model each kind uses. Choosing defaults is the backend's job;
// these helpers only look them up for display.

import type { Model, ModelKind, ModelKindId, ModelRef } from "../../api/types";

export type Defaults = Record<ModelKindId, ModelRef | null>;

export const sameRef = (a: ModelRef | null | undefined, b: ModelRef | null | undefined) =>
  !!a && !!b && a.connectionId === b.connectionId && a.modelId === b.modelId;

export const findModel = (models: Model[], ref: ModelRef | null | undefined) =>
  models.find((m) => sameRef({ connectionId: m.connectionId, modelId: m.id }, ref)) ?? null;

export type KindStatus = { kind: ModelKind; model: Model | null };

export function kindStatuses(kinds: ModelKind[], defaults: Defaults, models: Model[]): KindStatus[] {
  return kinds.map((kind) => ({ kind, model: findModel(models, defaults[kind.id]) }));
}

/** The kinds in `needed` that have no working model. */
export function missingKinds(needed: ModelKindId[], kinds: ModelKind[], defaults: Defaults, models: Model[]): ModelKind[] {
  return kinds.filter((k) => needed.includes(k.id) && !findModel(models, defaults[k.id]));
}
