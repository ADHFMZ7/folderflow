// The main window: the sidebar on the window chrome, and the current page on a
// raised sheet beside it.

import { useRoute } from "../../app/routes";
import { HistoryPage } from "../history/HistoryPage";
import { SettingsPage } from "../settings-page/SettingsPage";
import { TemplatesPage } from "../workflows/TemplatesPage";
import { Studio } from "../editor/Studio";
import { WorkflowsPage } from "../workflows/WorkflowsPage";
import { useWorkflows } from "../workflows/useWorkflows";
import { Sidebar } from "./Sidebar";
import { useSidebarCollapsed } from "./useSidebarCollapsed";
import styles from "./MainWindow.module.css";

export function MainWindow() {
  const route = useRoute();
  const workflows = useWorkflows();
  const needsYou = (workflows.workflows ?? []).reduce((n, w) => n + w.needsYou, 0);
  const sidebar = useSidebarCollapsed();

  return (
    // The chrome between the sidebar and the sheet moves the window when dragged.
    <div className={sidebar.collapsed ? `${styles.window} ${styles.collapsed}` : styles.window} data-tauri-drag-region>
      <Sidebar current={route.page === "workflow" ? "workflows" : route.page} needsYou={needsYou}
        collapsed={sidebar.collapsed} onToggle={sidebar.toggle} />
      <div className={styles.sheet}>
        {route.page === "workflow" ? (
          // The editor takes the whole sheet, with no page padding or scrolling of its own.
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
      </div>
    </div>
  );
}
