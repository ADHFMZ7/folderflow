// The Studio editor (layout A of the UX spec): palette on the left, canvas in the
// middle, inspector on the right. The draft workflow is the only source of truth;
// every canvas event becomes one of the edits in graph.ts.

import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
  Background, Controls, MiniMap, ReactFlow, ReactFlowProvider, useReactFlow,
  type Connection, type EdgeChange, type NodeChange,
} from "@xyflow/react";
import "@xyflow/react/dist/style.css";
import type { StepType, Workflow } from "../../api/types";
import { hrefFor } from "../../app/routes";
import { Button, Dialog, Spinner, TextInput, Toggle } from "../../ui";
import { addStep, connect, disconnect, isTrigger, moveStep, removeSteps, toFlow, updateStep } from "./graph";
import { Inspector } from "./Inspector";
import { Palette } from "./Palette";
import { StepNode } from "./StepNode";
import { useEditor } from "./useEditor";
import styles from "./Studio.module.css";

const nodeTypes = { step: StepNode };
const plural = (n: number, word: string) => `${n === 0 ? "No" : n} ${word}${n === 1 ? "" : "s"}`;

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

function Editor({ state, edit, save, reload }: Omit<ReturnType<typeof useEditor>, "state"> & { state: Ready }) {
  const { draft, problems, dirty, saving, saveError } = state;
  const rf = useReactFlow();
  const [selected, setSelected] = useState<string | null>(null);
  const [selectedEdge, setSelectedEdge] = useState<string | null>(null);
  // React Flow measures each card; it needs those sizes back to show them.
  const [measured, setMeasured] = useState<Record<string, { width: number; height: number }>>({});

  const leaving = useUnsavedGuard(dirty);
  useSaveShortcut(save);
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
        edit((w) => moveStep(w, c.id, position));
      }
      if (c.type === "select") setSelected((cur) => (c.selected ? c.id : cur === c.id ? null : cur));
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

  const stepTitle = (stepId: string) => draft.steps.find((s) => s.id === stepId)?.title ?? null;

  return (
    <div className={styles.studio}>
      <header className={styles.toolbar}>
        <a className={styles.back} href={hrefFor({ page: "workflows" })}>All workflows</a>
        <TextInput className={styles.name} aria-label="Workflow name" value={draft.name}
          onChange={(e) => { const name = e.target.value; edit((w: Workflow) => ({ ...w, name })); }} />
        <span className={dirty ? styles.unsaved : styles.saved}>{dirty ? "Unsaved changes" : "Saved"}</span>
        <span className={problems.length ? styles.problemCount : styles.muted}>{plural(problems.length, "problem")}</span>
        <span className={styles.spacer} />
        <Button variant="secondary" disabled title="Trying a workflow on a file comes with the engine that runs workflows.">Try on a file</Button>
        <span title="Turning workflows on comes with the engine that runs them.">
          <Toggle label="On" checked={draft.enabled} onChange={() => {}} disabled />
        </span>
        <Button onClick={save} disabled={!dirty || saving}>{saving ? "Saving…" : "Save"}</Button>
      </header>

      {saveError && (
        <div className={styles.alert} role="alert">
          <span>
            {saveError.conflict
              ? "This workflow was changed somewhere else since you opened it, so your changes weren't saved."
              : `Couldn't save: ${saveError.message}`}
          </span>
          {saveError.conflict && <Button variant="secondary" onClick={reload}>Load the saved version</Button>}
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
            onPaneClick={() => { setSelected(null); setSelectedEdge(null); }}
            fitView fitViewOptions={{ padding: 0.15, minZoom: 0.9, maxZoom: 1 }}
            proOptions={{ hideAttribution: true }}
          >
            <Background gap={20} />
            <Controls showInteractive={false} position="bottom-right" />
            <MiniMap pannable zoomable position="top-right" style={{ width: 140, height: 100 }} />
          </ReactFlow>
        </section>
        <Inspector workflow={draft} step={selectedStep} problems={problems} stepTitle={stepTitle}
          onChange={(step) => edit((w) => updateStep(w, step))}
          onDelete={(stepId) => { edit((w) => removeSteps(w, [stepId])); setSelected(null); }}
          onSelect={(stepId) => { setSelected(stepId); rf.fitView({ nodes: [{ id: stepId }], duration: 300, maxZoom: 1 }); }} />
      </div>

      {leaving.to && (
        <Dialog title="Leave without saving?" onClose={leaving.stay}
          actions={<>
            <Button variant="secondary" onClick={leaving.stay}>Keep editing</Button>
            <Button variant="danger" onClick={leaving.go}>Leave without saving</Button>
            <Button onClick={async () => { if (await save()) leaving.go(); else leaving.stay(); }}>Save and leave</Button>
          </>}>
          <p>Your changes to “{draft.name}” haven't been saved.</p>
        </Dialog>
      )}
    </div>
  );
}

/**
 * Holds in-app navigation while there are unsaved changes, so the editor can ask
 * first. Asking happens in the app: native confirm dialogs don't show in Tauri's
 * macOS window, where they silently answer "no".
 */
function useUnsavedGuard(dirty: boolean) {
  const [to, setTo] = useState<string | null>(null);

  useEffect(() => {
    if (!dirty) return;
    const onClick = (e: MouseEvent) => {
      const link = (e.target as Element | null)?.closest?.("a[href^='#/']");
      if (!link) return;
      e.preventDefault();
      e.stopPropagation();
      setTo(link.getAttribute("href"));
    };
    // Closing the window in a browser; Tauri's window doesn't ask.
    const onUnload = (e: BeforeUnloadEvent) => e.preventDefault();
    document.addEventListener("click", onClick, true);
    window.addEventListener("beforeunload", onUnload);
    return () => {
      document.removeEventListener("click", onClick, true);
      window.removeEventListener("beforeunload", onUnload);
    };
  }, [dirty]);

  const stay = useCallback(() => setTo(null), []);
  const go = useCallback(() => {
    if (to) window.location.hash = to;
    setTo(null);
  }, [to]);
  return { to, stay, go };
}

/** Cmd-S (Ctrl-S elsewhere) saves. */
function useSaveShortcut(save: () => unknown) {
  const latest = useRef(save);
  latest.current = save;
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === "s") {
        e.preventDefault();
        latest.current();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);
}
