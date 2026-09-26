import { hrefFor, type Page } from "../../app/routes";
import { Badge } from "../../ui";
import { ModelReadiness } from "../models/ModelReadiness";
import styles from "./Sidebar.module.css";

const ITEMS: { page: Page; label: string }[] = [
  { page: "workflows", label: "Workflows" },
  { page: "history", label: "History" },
  { page: "templates", label: "Templates" },
  { page: "settings", label: "Settings" },
];

type Props = { current: Page; needsYou: number; collapsed: boolean; onToggle: () => void };

/**
 * The page list. Collapsed, it shrinks to a rail with just the menu button and
 * the needs-you count; its links are removed, so Tab never lands on hidden ones.
 */
export function Sidebar({ current, needsYou, collapsed, onToggle }: Props) {
  return (
    <nav className={collapsed ? styles.rail : styles.sidebar} aria-label="Main">
      <button type="button" className={styles.menu} onClick={onToggle}
        aria-expanded={!collapsed} aria-controls="sidebar-pages" aria-label={collapsed ? "Show sidebar" : "Hide sidebar"}
        title={collapsed ? "Show sidebar" : "Hide sidebar"}>
        <svg width="18" height="18" viewBox="0 0 18 18" aria-hidden>
          <path d="M3 5h12M3 9h12M3 13h12" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" />
        </svg>
      </button>

      {collapsed ? (
        needsYou > 0 && (
          <span className={styles.railBadge} aria-label={`${needsYou} workflow${needsYou === 1 ? "" : "s"} needs you`}>
            <Badge tone="danger">{needsYou}</Badge>
          </span>
        )
      ) : (
        <>
          <ul id="sidebar-pages" className={styles.items}>
            {ITEMS.map((item) => (
              <li key={item.page}>
                <a href={hrefFor({ page: item.page })} className={item.page === current ? styles.current : styles.item}
                  aria-current={item.page === current ? "page" : undefined}>
                  {item.label}
                  {item.page === "workflows" && needsYou > 0 && <Badge tone="danger">{needsYou}</Badge>}
                </a>
              </li>
            ))}
          </ul>
          <span className={styles.spacer} />
          <ModelReadiness />
        </>
      )}
    </nav>
  );
}
