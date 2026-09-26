// Step types to add: drag one onto the canvas, or click it to add it below the
// others. Dragging uses pointer events, not the browser's drag and drop, which
// Tauri's window takes over for dropping files.

import { useEffect, useRef, useState, type PointerEvent as ReactPointerEvent } from "react";
import type { StepType } from "../../api/types";
import { infoFor, KIND_LABEL, STEP_CATALOG, type StepKind } from "./catalog";
import styles from "./Studio.module.css";

/** Pointer travel below this is a click, not a drag. */
const DRAG_THRESHOLD_PX = 4;

type Drag = { type: StepType; startX: number; startY: number; x: number; y: number; moved: boolean };

export function Palette({ onAdd, onDrop }: {
  onAdd: (type: StepType) => void;
  /** Called when a drag ends; the receiver decides whether the point is on the canvas. */
  onDrop: (type: StepType, point: { x: number; y: number }) => void;
}) {
  const [drag, setDrag] = useState<Drag | null>(null);
  const suppressClick = useRef(false);

  useEffect(() => {
    if (!drag) return;
    const onMove = (e: PointerEvent) => setDrag((d) => d && {
      ...d, x: e.clientX, y: e.clientY,
      moved: d.moved || Math.hypot(e.clientX - d.startX, e.clientY - d.startY) > DRAG_THRESHOLD_PX,
    });
    const onUp = (e: PointerEvent) => {
      setDrag(null);
      if (drag.moved || Math.hypot(e.clientX - drag.startX, e.clientY - drag.startY) > DRAG_THRESHOLD_PX) {
        suppressClick.current = true;
        onDrop(drag.type, { x: e.clientX, y: e.clientY });
      }
    };
    const onCancel = () => setDrag(null);
    window.addEventListener("pointermove", onMove);
    window.addEventListener("pointerup", onUp);
    window.addEventListener("pointercancel", onCancel);
    return () => {
      window.removeEventListener("pointermove", onMove);
      window.removeEventListener("pointerup", onUp);
      window.removeEventListener("pointercancel", onCancel);
    };
  }, [drag, onDrop]);

  const start = (type: StepType) => (e: ReactPointerEvent) => {
    if (e.button !== 0) return;
    e.preventDefault(); // no text selection while dragging
    setDrag({ type, startX: e.clientX, startY: e.clientY, x: e.clientX, y: e.clientY, moved: false });
  };

  return (
    <aside className={styles.palette} aria-label="Steps">
      {(Object.keys(KIND_LABEL) as StepKind[]).map((kind) => (
        <section key={kind}>
          <h4 className={styles.paletteTitle}>{KIND_LABEL[kind]}</h4>
          {STEP_CATALOG.filter((s) => s.kind === kind).map((s) => (
            <button key={s.type} type="button" className={`${styles.paletteItem} ${styles[`kind-${kind}`]}`}
              aria-label={`Add ${s.label}`} title={s.blurb} onPointerDown={start(s.type)}
              onClick={() => {
                if (suppressClick.current) { suppressClick.current = false; return; }
                onAdd(s.type);
              }}>
              <span className={styles.glyph} aria-hidden>{s.glyph}</span>
              {s.label}
            </button>
          ))}
        </section>
      ))}
      <p className={styles.hint}>Drag a step onto the canvas, or click to add it.</p>
      {drag?.moved && (
        <div className={`${styles.ghost} ${styles[`kind-${infoFor(drag.type).kind}`]}`} data-testid="drag-ghost"
          style={{ left: drag.x, top: drag.y }} aria-hidden>
          <span className={styles.glyph}>{infoFor(drag.type).glyph}</span>{infoFor(drag.type).label}
        </div>
      )}
    </aside>
  );
}
