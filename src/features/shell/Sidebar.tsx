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

export function Sidebar({ current, needsYou }: { current: Page; needsYou: number }) {
  return (
    <nav className={styles.sidebar} aria-label="Main">
      <ul className={styles.items}>
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
      <ModelReadiness />
    </nav>
  );
}
