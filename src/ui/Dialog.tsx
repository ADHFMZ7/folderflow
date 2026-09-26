// A modal dialog drawn by the app. Native alert/confirm dialogs don't show in
// Tauri's macOS window, so the app never relies on them.

import { useEffect, useId, useRef, type ReactNode } from "react";
import styles from "./Dialog.module.css";

export function Dialog({ title, children, actions, onClose }: {
  title: string; children?: ReactNode; actions: ReactNode; onClose: () => void;
}) {
  const titleId = useId();
  const box = useRef<HTMLDivElement>(null);

  useEffect(() => {
    box.current?.querySelector<HTMLElement>("button")?.focus();
    const onKey = (e: KeyboardEvent) => { if (e.key === "Escape") onClose(); };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose]);

  return (
    <div className={styles.backdrop} onMouseDown={(e) => { if (e.target === e.currentTarget) onClose(); }}>
      <div ref={box} className={styles.dialog} role="dialog" aria-modal="true" aria-labelledby={titleId}>
        <h2 id={titleId} className={styles.title}>{title}</h2>
        {children}
        <div className={styles.actions}>{actions}</div>
      </div>
    </div>
  );
}
