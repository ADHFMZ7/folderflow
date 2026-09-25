import type { Template, WorkflowSummary } from "../../api/types";
import { useSettings } from "../../settings/SettingsProvider";
import { Banner, Button, Spinner } from "../../ui";
import { missingKinds } from "../models/defaults";
import { TemplateCard } from "./TemplateCard";
import { WorkflowCard } from "./WorkflowCard";
import styles from "./workflows.module.css";

export function WorkflowsPage({ workflows, templates }: { workflows: WorkflowSummary[] | null; templates: Template[] }) {
  return (
    <div className={styles.page}>
      <header className={styles.head}>
        <h1>Workflows</h1>
        {/* The editor is the next step; until then there is nothing to open. */}
        <Button disabled title="The workflow editor comes next">+ New workflow</Button>
      </header>
      <MissingModelsBanner workflows={workflows ?? []} />
      {workflows === null ? <Spinner label="Loading workflows…" />
        : workflows.length ? (
          <div className={styles.grid}>{workflows.map((w) => <WorkflowCard key={w.id} workflow={w} />)}</div>
        ) : <EmptyState templates={templates.slice(0, 3)} />}
    </div>
  );
}

/** Warns about model kinds that saved workflows need, or any kind at all before the first workflow exists. */
function MissingModelsBanner({ workflows }: { workflows: WorkflowSummary[] }) {
  const { kinds, models, settings } = useSettings();
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

function EmptyState({ templates }: { templates: Template[] }) {
  return (
    <section className={styles.empty} aria-label="No workflows yet">
      <h2>No workflows yet</h2>
      <p className={styles.muted}>Start from a blank canvas, describe what you want, or copy a template.</p>
      <div className={styles.actions}>
        <Button disabled title="The workflow editor comes next">New workflow</Button>
        <Button variant="secondary" disabled title="The workflow editor comes next">✦ Describe it</Button>
      </div>
      <div className={styles.templates}>{templates.map((t) => <TemplateCard key={t.id} template={t} />)}</div>
    </section>
  );
}
