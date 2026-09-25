import styles from "./HistoryPage.module.css";

export function HistoryPage() {
  return (
    <div className={styles.page}>
      <h1>History</h1>
      <p className={styles.muted}>Runs will appear here once workflows are running.</p>
    </div>
  );
}
