// The main window: a title bar across the top, the sidebar on the window chrome
// below it, and the current page on a raised sheet beside the sidebar.

import { PanelLeft } from "lucide-react";
import { useEffect } from "react";
import { useApi } from "../../api/api";
import { useRoute } from "../../app/routes";
import { HistoryPage } from "../runs/HistoryPage";
import { RunPage } from "../runs/RunPage";
import { SettingsPage } from "../settings-page/SettingsPage";
import { TemplatesPage } from "../workflows/TemplatesPage";
import { Studio } from "../editor/Studio";
import { WorkflowsPage } from "../workflows/WorkflowsPage";
import { useWorkflows } from "../workflows/useWorkflows";
import { ActivityProvider } from "./activity";
import { Sidebar } from "./Sidebar";
import { TitleTools } from "./TitleTools";
import { useSidebarCollapsed } from "./useSidebarCollapsed";
import styles from "./MainWindow.module.css";

export function MainWindow() {
  const route = useRoute();
  const api = useApi();
  // "2 need you" in the menu bar opens a page here.
  useEffect(() => api.onNavigate((hash) => { window.location.hash = hash; }), [api]);
  const workflows = useWorkflows();
  const needsYou = (workflows.workflows ?? []).reduce((n, w) => n + w.needsYou, 0);
  const sidebar = useSidebarCollapsed();

  return (
    <ActivityProvider>
      {/* The chrome between the sidebar and the sheet moves the window when dragged. */}
      <div className={sidebar.collapsed ? `${styles.window} ${styles.collapsed}` : styles.window} data-tauri-drag-region>
        {/* The title bar, across the whole window: the traffic lights, then the
            sidebar button, which stays put whether the sidebar is open or not;
            on the right, runs and Pause all, the bell, and light or dark. */}
        <header className={styles.titleBar} aria-label="Title bar" data-tauri-drag-region>
          <span className={styles.lights} aria-hidden />
          <button type="button" className={styles.menu} onClick={sidebar.toggle}
            aria-expanded={!sidebar.collapsed} aria-controls="sidebar-pages"
            aria-label={sidebar.collapsed ? "Show sidebar" : "Hide sidebar"} title={sidebar.collapsed ? "Show sidebar" : "Hide sidebar"}>
            <PanelLeft size={17} strokeWidth={1.7} aria-hidden />
          </button>
          <TitleTools />
        </header>
        <Sidebar current={route.page === "workflow" ? "workflows" : route.page === "run" ? "history" : route.page}
          needsYou={needsYou} collapsed={sidebar.collapsed} />
        <div className={styles.sheet}>
          {route.page === "workflow" ? (
            // The editor takes the whole sheet, with no page padding or scrolling of its own.
            <main className={styles.editor}><Studio key={route.id} id={route.id} /></main>
          ) : (
            <main className={styles.page}>
              <div className={styles.content}>
                {route.page === "workflows" && <WorkflowsPage {...workflows} />}
                {route.page === "history" && <HistoryPage />}
                {route.page === "run" && <RunPage key={route.id} id={route.id} />}
                {route.page === "templates" && <TemplatesPage templates={workflows.templates} onUse={workflows.create} />}
                {route.page === "settings" && <SettingsPage />}
              </div>
            </main>
          )}
        </div>
      </div>
    </ActivityProvider>
  );
}
