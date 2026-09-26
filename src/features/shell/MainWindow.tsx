// The main window: a title bar across the top, the sidebar on the window chrome
// below it, and the current page on a raised sheet beside the sidebar.

import { PanelLeft } from "lucide-react";
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
      {/* The title bar, across the whole window: the traffic lights, then the
          sidebar button, which stays put whether the sidebar is open or not. */}
      <header className={styles.titleBar} data-tauri-drag-region>
        <span className={styles.lights} aria-hidden />
        <button type="button" className={styles.menu} onClick={sidebar.toggle}
          aria-expanded={!sidebar.collapsed} aria-controls="sidebar-pages"
          aria-label={sidebar.collapsed ? "Show sidebar" : "Hide sidebar"} title={sidebar.collapsed ? "Show sidebar" : "Hide sidebar"}>
          <PanelLeft size={17} strokeWidth={1.7} aria-hidden />
        </button>
      </header>
      <Sidebar current={route.page === "workflow" ? "workflows" : route.page} needsYou={needsYou} collapsed={sidebar.collapsed} />
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
