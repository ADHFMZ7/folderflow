// The main window: sidebar on the left, the current page on the right.

import { useRoute, type Page } from "../../app/routes";
import { Studio } from "../editor/Studio";
import { HistoryPage } from "../history/HistoryPage";
import { ModelReadiness } from "../models/ModelReadiness";
import { SettingsPage } from "../settings-page/SettingsPage";
import { TemplatesPage } from "../workflows/TemplatesPage";
import { WorkflowsPage } from "../workflows/WorkflowsPage";
import { useWorkflows } from "../workflows/useWorkflows";
import { useShell } from "../../prototype/shell";
import { Sidebar } from "./Sidebar";
import { useSidebarCollapsed } from "./useSidebarCollapsed";
import styles from "./MainWindow.module.css";

const TITLES: Record<Page, string> = { workflows: "Workflows", history: "History", templates: "Templates", settings: "Settings" };

export function MainWindow() {
  const route = useRoute();
  const workflows = useWorkflows();
  const needsYou = (workflows.workflows ?? []).reduce((n, w) => n + w.needsYou, 0);
  const sidebar = useSidebarCollapsed();
  const editing = route.page === "workflow";
  const unified = useShell() === "unified";

  return (
    <div className={sidebar.collapsed ? `${styles.window} ${styles.collapsed}` : styles.window}>
      <Sidebar current={editing ? "workflows" : route.page} needsYou={needsYou}
        collapsed={sidebar.collapsed} onToggle={sidebar.toggle} />
      <div className={styles.main}>
        {/* PROTOTYPE: the top bar and status bar show only in the unified style. */}
        {unified && !editing && (
          <header className={styles.topBar} data-tauri-drag-region>
            <span className={styles.crumb}>{TITLES[route.page as Page]}</span>
          </header>
        )}
        {editing ? (
          // The editor takes the whole area, with no page padding or scrolling of its own.
          <main className={styles.editor}><Studio key={route.id} id={route.id} /></main>
        ) : (
          <main className={styles.page}>
            <div className={styles.content}>
              {route.page === "workflows" && <WorkflowsPage {...workflows} />}
              {route.page === "history" && <HistoryPage />}
              {route.page === "templates" && <TemplatesPage templates={workflows.templates} onUse={workflows.create} />}
              {route.page === "settings" && <SettingsPage />}
            </div>
          </main>
        )}
        {unified && <footer className={styles.statusBar}>
          <ModelReadiness inline />
          <span className={styles.spacer} />
          {needsYou > 0 && <span>{needsYou} workflow{needsYou === 1 ? "" : "s"} need{needsYou === 1 ? "s" : ""} you</span>}
        </footer>}
      </div>
    </div>
  );
}
