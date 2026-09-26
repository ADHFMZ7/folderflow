import type { TextareaHTMLAttributes } from "react";
import styles from "./TextArea.module.css";

/** Multi-line text, styled like TextInput. */
export function TextArea({ className, rows = 3, ...rest }: TextareaHTMLAttributes<HTMLTextAreaElement>) {
  return <textarea rows={rows} className={[styles.area, className].filter(Boolean).join(" ")} {...rest} />;
}
