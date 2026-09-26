// The Studio editor (layout A of the UX spec): palette on the left, canvas in the
// middle, inspector on the right. The draft workflow is the only source of truth;
// every canvas event becomes one of the edits in graph.ts.

import { useCallback, useEffect, useMemo, useRef, useState, type DragEvent } from "react";
import {
  Background, Controls, MiniMap, ReactFlow, ReactFlowProvider, useReactFlow,
  type Connection, type EdgeChange, type NodeChange,
} from "@xyflow/react";
import "@xyflow/react/dist/style.css";
import type { StepType, Workflow } from "../../api/types";
import { hrefFor } from "../../app/routes";
import { Button, Spinner, TextInput, Toggle } from "../../ui";
import { addStep, connect, disconnect, isTrigger, moveStep, removeSteps, toFlow, updateStep } from "./graph";
import { Inspector } from "./Inspector";
import { DRAG_TYPE, Palette } from "./Palette";
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

  useUnsavedGuard(dirty);
  useSaveShortcut(save);

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

  const onDrop = (e: DragEvent) => {
    e.preventDefault();
    const type = e.dataTransfer.getData(DRAG_TYPE) as StepType;
    if (type) add(type, rf.screenToFlowPosition({ x: e.clientX, y: e.clientY }));
  };

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
        <Palette onAdd={addBelow} />
        <section className={styles.canvas} aria-label="Canvas" onDrop={onDrop} onDragOver={(e) => e.preventDefault()}>
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
        <Inspector step={selectedStep} problems={problems} stepTitle={stepTitle}
          onChange={(step) => edit((w) => updateStep(w, step))}
          onDelete={(stepId) => { edit((w) => removeSteps(w, [stepId])); setSelected(null); }}
          onSelect={(stepId) => { setSelected(stepId); rf.fitView({ nodes: [{ id: stepId }], duration: 300, maxZoom: 1 }); }} />
      </div>
    </div>
  );
}

/** Asks before leaving the editor through an in-app link, or closing the window, with unsaved changes. */
function useUnsavedGuard(dirty: boolean) {
  useEffect(() => {
    if (!dirty) return;
    const onClick = (e: MouseEvent) => {
      const link = (e.target as Element | null)?.closest?.("a[href^='#/']");
      if (link && !window.confirm("Leave without saving? Your changes to this workflow will be lost.")) {
        e.preventDefault();
        e.stopPropagation();
      }
    };
    const onUnload = (e: BeforeUnloadEvent) => e.preventDefault();
    document.addEventListener("click", onClick, true);
    window.addEventListener("beforeunload", onUnload);
    return () => {
      document.removeEventListener("click", onClick, true);
      window.removeEventListener("beforeunload", onUnload);
    };
  }, [dirty]);
}

/** Cmd-S (Ctrl-S elsewhere) saves. */
function useSaveShortcut(save: () => void) {
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
