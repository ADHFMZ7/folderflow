// The main window: sidebar on the left, the current page on the right.

import { useRoute } from "../../app/routes";
import { HistoryPage } from "../history/HistoryPage";
import { SettingsPage } from "../settings-page/SettingsPage";
import { TemplatesPage } from "../workflows/TemplatesPage";
import { WorkflowPage } from "../workflows/WorkflowPage";
import { WorkflowsPage } from "../workflows/WorkflowsPage";
import { useWorkflows } from "../workflows/useWorkflows";
import { Sidebar } from "./Sidebar";
import styles from "./MainWindow.module.css";

export function MainWindow() {
  const route = useRoute();
  const workflows = useWorkflows();
  const needsYou = (workflows.workflows ?? []).reduce((n, w) => n + w.needsYou, 0);

  return (
    <div className={styles.window}>
      <Sidebar current={route.page === "workflow" ? "workflows" : route.page} needsYou={needsYou} />
      <main className={styles.page}>
        <div className={styles.content}>
          {route.page === "workflows" && <WorkflowsPage {...workflows} />}
          {route.page === "workflow" && <WorkflowPage key={route.id} id={route.id} />}
          {route.page === "history" && <HistoryPage />}
          {route.page === "templates" && <TemplatesPage templates={workflows.templates} onUse={workflows.create} />}
          {route.page === "settings" && <SettingsPage />}
        </div>
      </main>
    </div>
  );
}
