import type { InputHTMLAttributes, SelectHTMLAttributes } from "react";
import styles from "./TextInput.module.css";

export function TextInput({ className, ...rest }: InputHTMLAttributes<HTMLInputElement>) {
  return <input className={[styles.input, className].filter(Boolean).join(" ")} {...rest} />;
}

export function Select({ className, ...rest }: SelectHTMLAttributes<HTMLSelectElement>) {
  return <select className={[styles.input, className].filter(Boolean).join(" ")} {...rest} />;
}
