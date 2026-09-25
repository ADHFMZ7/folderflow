import type { ButtonHTMLAttributes } from "react";
import styles from "./Button.module.css";

type Props = ButtonHTMLAttributes<HTMLButtonElement> & {
  variant?: "primary" | "secondary" | "danger" | "quiet";
};

export function Button({ variant = "primary", className, type = "button", ...rest }: Props) {
  return <button type={type} className={[styles.button, styles[variant], className].filter(Boolean).join(" ")} {...rest} />;
}
