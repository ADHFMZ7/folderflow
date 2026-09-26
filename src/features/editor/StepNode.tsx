// A step on the canvas: its kind, title and problems, an input on top, and one
// output per exit along the bottom, labelled with the branch it stands for.

import { Handle, Position, type NodeProps } from "@xyflow/react";
import { exitsOf, isTrigger, type StepNode as StepNodeType } from "./graph";
import { infoFor } from "./catalog";
import styles from "./Studio.module.css";

export function StepNode({ data: { step, problems }, selected }: NodeProps<StepNodeType>) {
  const info = infoFor(step.type);
  const exits = exitsOf(step);
  const cls = [styles.node, styles[`kind-${info.kind}`], selected && styles.selected, problems.length > 0 && styles.hasProblem]
    .filter(Boolean).join(" ");

  return (
    <div className={cls}>
      {!isTrigger(step.type) && <Handle type="target" position={Position.Top} />}
      <div className={styles.nodeHead}>
        <span className={styles.glyph} aria-hidden>{info.glyph}</span>
        <span className={styles.nodeKind}>{info.label}</span>
        {problems.length > 0 && <span className={styles.problemDot} title={problems.map((p) => p.message).join("\n")}>{problems.length}</span>}
      </div>
      <div className={styles.nodeTitle}>{step.title}</div>
      {problems[0] && <div className={styles.nodeProblem}>{problems[0].message}</div>}
      {exits.length > 1 && (
        <div className={styles.branchLabels} aria-hidden>
          {exits.map((e) => <span key={e.handle}>{e.label}</span>)}
        </div>
      )}
      {exits.map((exit, i) => (
        <Handle key={exit.handle ?? "next"} id={exit.handle ?? undefined} type="source" position={Position.Bottom}
          style={exits.length > 1 ? { left: `${((i + 0.5) / exits.length) * 100}%` } : undefined} />
      ))}
    </div>
  );
}
