import { History, LayoutTemplate, Settings, Workflow, type LucideIcon } from "lucide-react";
import { hrefFor, type Page } from "../../app/routes";
import { Badge } from "../../ui";
import { ModelReadiness } from "../models/ModelReadiness";
import styles from "./Sidebar.module.css";

const ITEMS: { page: Page; label: string; icon: LucideIcon }[] = [
  { page: "workflows", label: "Workflows", icon: Workflow },
  { page: "history", label: "History", icon: History },
  { page: "templates", label: "Templates", icon: LayoutTemplate },
  { page: "settings", label: "Settings", icon: Settings },
];

type Props = { current: Page; needsYou: number; collapsed: boolean };

/**
 * The page list, on the window's chrome, below the title bar. Collapsed, it's a
 * rail of icons whose links keep their names for screen readers and tooltips.
 * The button that collapses it lives in the title bar, so it never moves.
 */
export function Sidebar({ current, needsYou, collapsed }: Props) {
  const needsLabel = `${needsYou} workflow${needsYou === 1 ? "" : "s"} needs you`;
  return (
    <nav className={collapsed ? styles.rail : styles.sidebar} aria-label="Main">
      <ul id="sidebar-pages" className={styles.items}>
        {ITEMS.map(({ page, label, icon: Icon }) => (
          <li key={page}>
            <a href={hrefFor({ page })} className={page === current ? styles.current : styles.item}
              aria-current={page === current ? "page" : undefined}
              aria-label={collapsed ? label : undefined} title={collapsed ? label : undefined}>
              <Icon size={17} strokeWidth={1.7} aria-hidden className={styles.icon} />
              {!collapsed && <span className={styles.label}>{label}</span>}
              {page === "workflows" && needsYou > 0 && (
                collapsed
                  ? <span className={styles.dot} aria-label={needsLabel}>{needsYou}</span>
                  : <Badge tone="danger">{needsYou}</Badge>
              )}
            </a>
          </li>
        ))}
      </ul>
      <span className={styles.spacer} />
      {!collapsed && <div className={styles.readiness}><ModelReadiness /></div>}
    </nav>
  );
}
