// Step types to add: drag one onto the canvas, or click it to add it below the others.

import type { StepType } from "../../api/types";
import { KIND_LABEL, STEP_CATALOG, type StepKind } from "./catalog";
import styles from "./Studio.module.css";

export const DRAG_TYPE = "application/x-folderflow-step";

export function Palette({ onAdd }: { onAdd: (type: StepType) => void }) {
  return (
    <aside className={styles.palette} aria-label="Steps">
      {(Object.keys(KIND_LABEL) as StepKind[]).map((kind) => (
        <section key={kind}>
          <h4 className={styles.paletteTitle}>{KIND_LABEL[kind]}</h4>
          {STEP_CATALOG.filter((s) => s.kind === kind).map((s) => (
            <button key={s.type} type="button" className={`${styles.paletteItem} ${styles[`kind-${kind}`]}`}
              aria-label={`Add ${s.label}`} title={s.blurb} draggable
              onDragStart={(e) => e.dataTransfer.setData(DRAG_TYPE, s.type)} onClick={() => onAdd(s.type)}>
              <span className={styles.glyph} aria-hidden>{s.glyph}</span>
              {s.label}
            </button>
          ))}
        </section>
      ))}
      <p className={styles.hint}>Drag a step onto the canvas, or click to add it.</p>
    </aside>
  );
}
