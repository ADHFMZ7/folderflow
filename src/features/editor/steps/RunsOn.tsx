// Which model an AI step runs on, and where the file's contents go: from the
// default model for the step's kind, and the provider's own privacy line.

import type { StepType } from "../../../api/types";
import { hrefFor } from "../../../app/routes";
import { useSettings } from "../../../settings/SettingsProvider";
import styles from "./Steps.module.css";

const KIND_OF: Partial<Record<StepType, string>> = { classify: "system1", extract: "llm", write: "llm", agent: "llm" };

export function RunsOn({ type }: { type: StepType }) {
  const { settings, kinds, providers, models } = useSettings();
  const kindId = KIND_OF[type];
  if (!kindId) return null;
  const kindName = kinds.find((k) => k.id === kindId)?.name ?? kindId;
  const ref = settings.defaults[kindId];
  const connection = ref && settings.connections.find((c) => c.id === ref.connectionId);
  const provider = connection && providers.find((p) => p.id === connection.providerId);
  if (!ref || !provider) {
    return (
      <p className={styles.runsOn}>
        No default {kindName} model yet. <a href={hrefFor({ page: "settings" })}>Choose one in Settings</a>
      </p>
    );
  }
  const model = models.find((m) => m.connectionId === ref.connectionId && m.id === ref.modelId)?.name ?? ref.modelId;
  return (
    <p className={styles.runsOn}>
      {`Runs on ${model} from ${provider.name}. ${provider.privacy}`}
    </p>
  );
}
