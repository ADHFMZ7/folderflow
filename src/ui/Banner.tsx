import type { ReactNode } from "react";
import styles from "./Banner.module.css";

export function Banner({ tone = "warn", children, action }: { tone?: "warn" | "info"; children: ReactNode; action?: ReactNode }) {
  return (
    <div className={`${styles.banner} ${styles[tone]}`} role="status">
      <div>{children}</div>
      {action}
    </div>
  );
}
