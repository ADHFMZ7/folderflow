import type { Template, WorkflowSummary } from "../../api/types";
import { useSettings } from "../../settings/SettingsProvider";
import { Banner, Button, Spinner } from "../../ui";
import { missingKinds } from "../models/defaults";
import { TemplateCard } from "./TemplateCard";
import { WorkflowCard } from "./WorkflowCard";
import styles from "./workflows.module.css";

type Props = {
  workflows: WorkflowSummary[] | null;
  templates: Template[];
  create: (templateId: string | null) => Promise<void>;
  remove: (id: string) => Promise<void>;
};

export function WorkflowsPage({ workflows, templates, create, remove }: Props) {
  return (
    <div className={styles.page}>
      <header className={styles.head}>
        <h1>Workflows</h1>
        <Button onClick={() => create(null)}>+ New workflow</Button>
      </header>
      <MissingModelsBanner workflows={(workflows ?? []).filter((w) => w.status === "ok")} />
      {workflows === null ? <Spinner label="Loading workflows…" />
        : workflows.length ? (
          <div className={styles.grid}>{workflows.map((w) => <WorkflowCard key={w.id} workflow={w} onDelete={remove} />)}</div>
        ) : <EmptyState templates={templates.slice(0, 3)} onUse={create} />}
    </div>
  );
}

/** Warns about model kinds that saved workflows need, or any kind at all before the first workflow exists.
    A provider that couldn't be reached is reported first, since it explains the missing models. */
function MissingModelsBanner({ workflows }: { workflows: WorkflowSummary[] }) {
  const { kinds, models, settings, modelProblems, retryModels } = useSettings();
  if (modelProblems.length) {
    const [first] = modelProblems;
    return (
      <Banner action={<Button variant="secondary" onClick={retryModels}>Retry</Button>}>
        Couldn't load models from {modelProblems.map((p) => p.providerName).join(" and ")}. {first.message}
      </Banner>
    );
  }
  const needed = workflows.length ? [...new Set(workflows.flatMap((w) => w.kindsNeeded))] : kinds.map((k) => k.id);
  const missing = missingKinds(needed, kinds, settings.defaults, models);
  if (!missing.length) return null;

  const names = missing.map((k) => k.name).join(" and ");
  const steps = missing.flatMap((k) => k.usedBy).join(", ");
  return (
    <Banner action={<a className={styles.bannerLink} href="#/settings">Set up models</a>}>
      No {names} model is set up, so {steps} steps won't run.
    </Banner>
  );
}

function EmptyState({ templates, onUse }: { templates: Template[]; onUse: (templateId: string | null) => void }) {
  return (
    <section className={styles.empty} aria-label="No workflows yet">
      <h2>No workflows yet</h2>
      <p className={styles.muted}>Start from a blank canvas, or copy a template and change it.</p>
      <div className={styles.actions}>
        <Button variant="secondary" disabled title="Describing a workflow in words comes later">✦ Describe it</Button>
      </div>
      <div className={styles.templates}>{templates.map((t) => <TemplateCard key={t.id} template={t} onUse={onUse} />)}</div>
    </section>
  );
}
