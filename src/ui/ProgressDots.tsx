import styles from "./ProgressDots.module.css";

export function ProgressDots({ count, current }: { count: number; current: number }) {
  return (
    <div className={styles.dots} role="img" aria-label={`Step ${current + 1} of ${count}`}>
      {Array.from({ length: count }, (_, i) => <span key={i} className={i === current ? styles.on : styles.dot} />)}
    </div>
  );
}
