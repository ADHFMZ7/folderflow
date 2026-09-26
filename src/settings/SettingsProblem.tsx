// Shown instead of the app when settings can't be used at all.

import type { ApiError } from "../api/types";
import styles from "./SettingsProblem.module.css";

export function SettingsProblem({ error }: { error: ApiError }) {
  const tooNew = error.code === "too_new";
  return (
    <main className={styles.screen}>
      <div className="window-drag-strip" data-tauri-drag-region />
      <section className={styles.card} aria-labelledby="problem-title">
        <h1 id="problem-title">{tooNew ? "These settings need a newer FolderFlow" : "FolderFlow couldn't open its settings"}</h1>
        <p className={styles.body}>
          {tooNew
            ? "Your settings were saved by a newer version of FolderFlow. To keep them safe, this version won't read or change them. Open the newer version to carry on."
            : "Something went wrong while reading your settings. Nothing was changed."}
        </p>
        <p className={styles.detail}>{error.message}</p>
      </section>
    </main>
  );
}
