import { openUrl } from "@tauri-apps/plugin-opener";
import type { ReactNode } from "react";
import styles from "./ExternalLink.module.css";

const inTauri = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

/** Opens in the user's browser: through Tauri inside the app, a new tab elsewhere. */
export function ExternalLink({ href, children }: { href: string; children: ReactNode }) {
  return (
    <a className={styles.link} href={href} target="_blank" rel="noreferrer"
      onClick={(e) => { if (inTauri) { e.preventDefault(); openUrl(href); } }}>
      {children} <span aria-hidden>↗</span>
    </a>
  );
}
