import styles from "./Toggle.module.css";

export function Toggle({ checked, onChange, label, disabled }: {
  checked: boolean; onChange: (checked: boolean) => void; label: string; disabled?: boolean;
}) {
  return (
    <label className={styles.toggle}>
      <input type="checkbox" role="switch" checked={checked} disabled={disabled} onChange={(e) => onChange(e.target.checked)} />
      <span className={styles.track} aria-hidden><span className={styles.thumb} /></span>
      <span>{label}</span>
    </label>
  );
}
