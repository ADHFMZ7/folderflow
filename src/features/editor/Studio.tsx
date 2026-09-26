// The Studio editor (layout A of the UX spec): palette on the left and the canvas
// beside it. A card floats over the canvas with the selected step's settings, or
// the problems when asked; with neither, the canvas has the room to itself. The draft workflow is the only source of truth;
// every canvas event becomes one of the edits in graph.ts.

import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
  Background, Controls, MiniMap, ReactFlow, ReactFlowProvider, useReactFlow,
  type Connection, type EdgeChange, type NodeChange,
} from "@xyflow/react";
import "@xyflow/react/dist/style.css";
import { Redo2, Undo2 } from "lucide-react";
import type { StepType, Workflow } from "../../api/types";
import { hrefFor } from "../../app/routes";
import { Button, Dialog, Spinner, TextInput, Toggle } from "../../ui";
import { addStep, connect, disconnect, isTrigger, moveStep, removeSteps, toFlow, updateStep } from "./graph";
import { Inspector } from "./Inspector";
import { Palette } from "./Palette";
import { StepNode } from "./StepNode";
import { useEditor, type SaveStatus } from "./useEditor";
import styles from "./Studio.module.css";

const nodeTypes = { step: StepNode };
const plural = (n: number, word: string) => `${n === 0 ? "No" : n} ${word}${n === 1 ? "" : "s"}`;
/** How long "Saved" stays up after a save. */
const SAVED_FLASH_MS = 2000;

export function Studio({ id }: { id: string }) {
  const editor = useEditor(id);
  const { state } = editor;

  if (state.status === "loading") return <div className={styles.center}><Spinner label="Opening workflow…" /></div>;
  if (state.status === "failed") {
    return (
      <div className={styles.center}>
        <a className={styles.back} href={hrefFor({ page: "workflows" })}>All workflows</a>
        <p>{state.message}</p>
      </div>
    );
  }
  return <ReactFlowProvider><Editor {...editor} state={state} /></ReactFlowProvider>;
}

type Ready = Extract<ReturnType<typeof useEditor>["state"], { status: "ready" }>;

function Editor({ state, edit, undo, redo, flush, apply, discard, reload }: Omit<ReturnType<typeof useEditor>, "state"> & { state: Ready }) {
  const { draft, problems, save, pendingApply, past, future } = state;
  const rf = useReactFlow();
  const [selected, setSelected] = useState<string | null>(null);
  const [selectedEdge, setSelectedEdge] = useState<string | null>(null);
  const [showProblems, setShowProblems] = useState(false);
  // React Flow measures each card; it needs those sizes back to show them.
  const [measured, setMeasured] = useState<Record<string, { width: number; height: number }>>({});

  const leaving = useLeaveGuard(save.kind, flush);
  useShortcuts({ save: flush, undo, redo });
  const canvasRef = useRef<HTMLElement>(null);

  const flow = useMemo(() => toFlow(draft, problems), [draft, problems]);
  const nodes = flow.nodes.map((n) => ({ ...n, measured: measured[n.id], selected: n.id === selected }));
  const edges = flow.edges.map((e) => ({ ...e, selected: e.id === selectedEdge }));
  const selectedStep = draft.steps.find((s) => s.id === selected) ?? null;

  const onNodesChange = useCallback((changes: NodeChange[]) => {
    const removed: string[] = [];
    for (const c of changes) {
      if (c.type === "dimensions" && c.dimensions) {
        const size = c.dimensions;
        setMeasured((m) => ({ ...m, [c.id]: size }));
      }
      if (c.type === "position" && c.position) {
        const position = c.position;
        edit((w) => moveStep(w, c.id, position), `move:${c.id}`);
      }
      if (c.type === "select") {
        setSelected((cur) => (c.selected ? c.id : cur === c.id ? null : cur));
        if (c.selected) setShowProblems(false);
      }
      if (c.type === "remove") removed.push(c.id);
    }
    if (removed.length) edit((w) => removeSteps(w, removed));
  }, [edit]);

  const onEdgesChange = useCallback((changes: EdgeChange[]) => {
    for (const c of changes) {
      if (c.type === "select") setSelectedEdge((cur) => (c.selected ? c.id : cur === c.id ? null : cur));
      if (c.type === "remove") {
        const edge = flow.edges.find((e) => e.id === c.id);
        if (edge) edit((w) => disconnect(w, edge.source, edge.sourceHandle ?? null));
      }
    }
  }, [edit, flow.edges]);

  const onConnect = useCallback((c: Connection) => {
    edit((w) => connect(w, c.source, c.sourceHandle ?? null, c.target));
  }, [edit]);

  const add = useCallback((type: StepType, position: { x: number; y: number }) => {
    let newId = "";
    edit((w) => {
      const result = addStep(w, type, position);
      newId = result.id;
      return result.workflow;
    });
    setSelected(newId);
    setSelectedEdge(null);
  }, [edit]);

  /** A clicked palette item goes below the lowest step. */
  const addBelow = (type: StepType) => {
    const lowest = draft.steps.reduce((y, s) => Math.max(y, s.position.y), -160);
    add(type, { x: selectedStep?.position.x ?? draft.steps[0]?.position.x ?? 0, y: lowest + 160 });
  };

  /** A palette drag that ends on the canvas adds the step where it was dropped. */
  const onDrop = useCallback((type: StepType, point: { x: number; y: number }) => {
    const r = canvasRef.current?.getBoundingClientRect();
    if (!r || point.x < r.left || point.x > r.right || point.y < r.top || point.y > r.bottom) return;
    add(type, rf.screenToFlowPosition(point));
  }, [add, rf]);

  // The card covers the right of the canvas; slide a newly selected step out from under it.
  useEffect(() => {
    if (!selected) return;
    const frame = requestAnimationFrame(() => {
      const card = canvasRef.current?.querySelector("aside");
      const node = canvasRef.current?.querySelector(`.react-flow__node[data-id="${CSS.escape(selected)}"]`);
      if (!card || !node) return;
      const overlap = node.getBoundingClientRect().right - card.getBoundingClientRect().left + 24;
      if (overlap <= 0) return;
      const { x, y, zoom } = rf.getViewport();
      rf.setViewport({ x: x - overlap, y, zoom }, { duration: 200 });
    });
    return () => cancelAnimationFrame(frame);
  }, [selected, rf]);

  const stepTitle = (stepId: string) => draft.steps.find((s) => s.id === stepId)?.title ?? null;

  return (
    <div className={styles.studio}>
      <header className={styles.toolbar} data-tauri-drag-region>
        <a className={styles.back} href={hrefFor({ page: "workflows" })}>All workflows</a>
        <TextInput className={styles.name} aria-label="Workflow name" value={draft.name}
          onChange={(e) => { const name = e.target.value; edit((w: Workflow) => ({ ...w, name }), "name"); }} />
        <span className={styles.history}>
          <button type="button" className={styles.icon} aria-label="Undo" title="Undo (⌘Z)" onClick={undo} disabled={!past.length}>
            <Undo2 size={17} strokeWidth={1.8} aria-hidden />
          </button>
          <button type="button" className={styles.icon} aria-label="Redo" title="Redo (⇧⌘Z)" onClick={redo} disabled={!future.length}>
            <Redo2 size={17} strokeWidth={1.8} aria-hidden />
          </button>
        </span>
        <SaveIndicator kind={save.kind} />
        {problems.length > 0 && (
          <button type="button" className={styles.problemCount} onClick={() => { setSelected(null); setShowProblems(true); }}>
            {plural(problems.length, "problem")}
          </button>
        )}
        <span className={styles.spacer} />
        <Button variant="secondary" disabled title="Trying a workflow on a file comes with the engine that runs workflows.">Try on a file</Button>
        <span title="Turning workflows on comes with the engine that runs them.">
          <Toggle label="On" checked={draft.enabled} onChange={() => {}} disabled />
        </span>
      </header>

      {save.kind === "failed" && (
        <div className={styles.alert} role="alert">
          <span>
            {save.conflict
              ? "This workflow was changed somewhere else since you opened it, so your latest changes weren't saved."
              : `Couldn't save your changes: ${save.message}`}
          </span>
          {save.conflict
            ? <Button variant="secondary" onClick={reload}>Load the latest version</Button>
            : <Button variant="secondary" onClick={() => flush()}>Try again</Button>}
        </div>
      )}

      {pendingApply && (
        <div className={styles.draftBar} role="status" aria-label="Changes not live">
          <span>You have changes that aren't live yet. The running version stays as it is until you apply them.</span>
          <Button variant="secondary" onClick={discard}>Discard changes</Button>
          <Button onClick={apply} disabled={problems.length > 0}
            title={problems.length ? `Fix ${plural(problems.length, "problem").toLowerCase()} first` : undefined}>Apply</Button>
        </div>
      )}

      <div className={styles.body}>
        <Palette onAdd={addBelow} onDrop={onDrop} />
        <section ref={canvasRef} className={styles.canvas} aria-label="Canvas">
          <ReactFlow
            nodes={nodes} edges={edges} nodeTypes={nodeTypes}
            onNodesChange={onNodesChange} onEdgesChange={onEdgesChange} onConnect={onConnect}
            isValidConnection={(c) => {
              const target = draft.steps.find((s) => s.id === c.target);
              return !!target && !isTrigger(target.type) && c.source !== c.target;
            }}
            onPaneClick={() => { setSelected(null); setSelectedEdge(null); setShowProblems(false); }}
            fitView fitViewOptions={{ padding: 0.15, minZoom: 0.6, maxZoom: 1 }}
            proOptions={{ hideAttribution: true }}
          >
            <Background gap={20} />
            <Controls showInteractive={false} position="bottom-left" />
            <MiniMap pannable zoomable position="top-left" style={{ width: 140, height: 100 }} />
          </ReactFlow>
          {(selectedStep || showProblems) && (
            <Inspector workflow={draft} step={selectedStep} problems={problems} stepTitle={stepTitle}
              onChange={(step) => edit((w) => updateStep(w, step), `step:${step.id}`)}
              onDelete={(stepId) => { edit((w) => removeSteps(w, [stepId])); setSelected(null); }}
              onSelect={(stepId) => { setShowProblems(false); setSelected(stepId); rf.fitView({ nodes: [{ id: stepId }], maxZoom: 1 }); }}
              onClose={() => { setSelected(null); setShowProblems(false); }} />
          )}
        </section>
      </div>

      {leaving.failed && (
        <Dialog title="Your last changes aren't saved" onClose={leaving.stay}
          actions={<>
            <Button variant="secondary" onClick={leaving.stay}>Keep editing</Button>
            <Button variant="danger" onClick={leaving.go}>Leave anyway</Button>
          </>}>
          <p>{save.kind === "failed" ? save.message : "Saving didn't finish."} If you leave now, those changes are lost.</p>
        </Dialog>
      )}
    </div>
  );
}

/**
 * Quiet while there's nothing to report: "Saving…" while changes wait to be
 * saved, "Saved" for a moment after, and "Not saved" until a failed save works.
 */
function SaveIndicator({ kind }: { kind: SaveStatus["kind"] }) {
  const [flash, setFlash] = useState(false);
  const previous = useRef(kind);
  useEffect(() => {
    const before = previous.current;
    previous.current = kind;
    if (kind !== "saved" || before === "saved") return;
    setFlash(true);
    const timer = window.setTimeout(() => setFlash(false), SAVED_FLASH_MS);
    return () => window.clearTimeout(timer);
  }, [kind]);

  const text = kind === "failed" ? "Not saved" : kind !== "saved" ? "Saving…" : flash ? "Saved" : null;
  return (
    <span className={kind === "failed" ? styles.notSaved : kind === "saved" ? styles.savedFlash : styles.muted} aria-live="polite">
      {text}
    </span>
  );
}

/**
 * Holds in-app navigation until pending changes are saved. Only if saving fails
 * does it ask, in the app: native confirm dialogs don't show in Tauri's macOS
 * window, where they silently answer "no".
 */
function useLeaveGuard(saveKind: string, flush: () => Promise<boolean>) {
  const [held, setHeld] = useState<{ to: string; failed: boolean } | null>(null);

  useEffect(() => {
    if (saveKind === "saved") return;
    const onClick = (e: MouseEvent) => {
      const link = (e.target as Element | null)?.closest?.("a[href^='#/']");
      if (!link) return;
      e.preventDefault();
      e.stopPropagation();
      const to = link.getAttribute("href")!;
      setHeld({ to, failed: false });
      flush().then((ok) => {
        if (ok) { window.location.hash = to; setHeld(null); } else setHeld({ to, failed: true });
      });
    };
    // Closing the window in a browser; Tauri's window doesn't ask.
    const onUnload = (e: BeforeUnloadEvent) => e.preventDefault();
    document.addEventListener("click", onClick, true);
    window.addEventListener("beforeunload", onUnload);
    return () => {
      document.removeEventListener("click", onClick, true);
      window.removeEventListener("beforeunload", onUnload);
    };
  }, [saveKind, flush]);

  const stay = useCallback(() => setHeld(null), []);
  const go = useCallback(() => {
    if (held) window.location.hash = held.to;
    setHeld(null);
  }, [held]);
  return { failed: !!held?.failed, stay, go };
}

/** ⌘S saves now, ⌘Z undoes, ⇧⌘Z (or ⌘Y) redoes; Ctrl on other systems. */
function useShortcuts(actions: { save: () => unknown; undo: () => void; redo: () => void }) {
  const latest = useRef(actions);
  latest.current = actions;
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (!(e.metaKey || e.ctrlKey)) return;
      const key = e.key.toLowerCase();
      if (key === "s") { e.preventDefault(); latest.current.save(); }
      else if (key === "z" && e.shiftKey) { e.preventDefault(); latest.current.redo(); }
      else if (key === "z") { e.preventDefault(); latest.current.undo(); }
      else if (key === "y") { e.preventDefault(); latest.current.redo(); }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);
}
