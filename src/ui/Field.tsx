import type { ReactNode } from "react";
import styles from "./Field.module.css";

/** A labelled control. The control sits inside the label, so no ids are needed; the hint
    stays outside it so links in a hint don't become part of the control's name. */
export function Field({ label, hint, children }: { label: string; hint?: ReactNode; children: ReactNode }) {
  return (
    <div className={styles.field}>
      <label className={styles.control}>
        <span className={styles.label}>{label}</span>
        {children}
      </label>
      {hint && <span className={styles.hint}>{hint}</span>}
    </div>
  );
}
