// PROTOTYPE: throwaway. Two ways to make the window read as one piece, next to
// the current look, switchable with ?shell=current|sheet|unified or the bar at
// the bottom. Components style each option under <html data-shell="…">.

import { useEffect, useState } from "react";
import { inTauri } from "../api/tauri";
import styles from "./shell.module.css";

export const SHELLS = [
  { key: "current", name: "Current" },
  { key: "sheet", name: "A · Sheet (like Flow)" },
  { key: "unified", name: "B · Unified (like Obsidian)" },
] as const;
export type Shell = (typeof SHELLS)[number]["key"];

function initialShell(): Shell {
  const asked = new URLSearchParams(location.search).get("shell");
  let saved: string | null = null;
  try { saved = localStorage.getItem("folderflow.prototype.shell"); } catch { /* ignore */ }
  const pick = asked ?? saved;
  return SHELLS.some((s) => s.key === pick) ? (pick as Shell) : "sheet";
}

/** Sets the shell before the first render, so there's no flash of the wrong look. */
export function applyInitialShell() {
  document.documentElement.dataset.shell = initialShell();
}

export function ShellSwitcher() {
  const [shell, setShell] = useState<Shell>(initialShell);
  useEffect(() => {
    document.documentElement.dataset.shell = shell;
    try { localStorage.setItem("folderflow.prototype.shell", shell); } catch { /* ignore */ }
    const url = new URL(location.href);
    url.searchParams.set("shell", shell);
    history.replaceState(null, "", url);
  }, [shell]);

  if (!import.meta.env.DEV) return null;
  const i = SHELLS.findIndex((s) => s.key === shell);
  const go = (step: number) => setShell(SHELLS[(i + step + SHELLS.length) % SHELLS.length].key);
  return (
    <div className={styles.switcher} aria-label="Prototype: window style">
      <button type="button" onClick={() => go(-1)} aria-label="Previous style">←</button>
      <span>{SHELLS[i].name}</span>
      <button type="button" onClick={() => go(1)} aria-label="Next style">→</button>
    </div>
  );
}

/**
 * macOS draws the real traffic lights over the window's top-left corner once the
 * title bar is an overlay (see tauri.conf.json). In a browser we draw stand-ins
 * so the prototype shows the same corner.
 */
export function TrafficLights() {
  // In the app the real ones are there already; keep their space.
  if (inTauri()) return <span className={styles.lightsSpace} aria-hidden />;
  return (
    <span className={styles.lights} aria-hidden>
      <i style={{ background: "#ff5f57" }} /><i style={{ background: "#febc2e" }} /><i style={{ background: "#28c840" }} />
    </span>
  );
}

/** The current window style, following changes from the switcher. */
export function useShell(): string | undefined {
  const [shell, setShell] = useState(() => document.documentElement.dataset.shell);
  useEffect(() => {
    const observer = new MutationObserver(() => setShell(document.documentElement.dataset.shell));
    observer.observe(document.documentElement, { attributes: true, attributeFilter: ["data-shell"] });
    return () => observer.disconnect();
  }, []);
  return shell;
}
