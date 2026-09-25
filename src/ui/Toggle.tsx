import styles from "./Toggle.module.css";

export function Toggle({ checked, onChange, label }: {
  checked: boolean; onChange: (checked: boolean) => void; label: string;
}) {
  return (
    <label className={styles.toggle}>
      <input type="checkbox" role="switch" checked={checked} onChange={(e) => onChange(e.target.checked)} />
      <span className={styles.track} aria-hidden><span className={styles.thumb} /></span>
      <span>{label}</span>
    </label>
  );
}
