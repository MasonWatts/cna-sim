// Time hierarchy over the event stream: Month → Week (Game-Turn) → Op-Stage → Phase → Step.
// The recorder stamps every event with game_turn / operations_stage / phase, so the whole hierarchy
// is derivable here — no re-recording needed.

import type { Event } from "@bindings/Event";

export type Level = "week" | "stage" | "phase" | "step";
export const LEVELS: Level[] = ["week", "stage", "phase", "step"];
export const LEVEL_LABEL: Record<Level, string> = {
  week: "Week",
  stage: "Op-Stage",
  phase: "Phase",
  step: "Step",
};

export interface GameDate {
  year: number;
  month: number; // 1–12
  week: number; // 1–4
}

// Mirror of engine `date_of` (crates/engine/src/state/game.rs): GT1 = Sep wk3 1940, GT2 = Sep wk4,
// then four one-week turns per calendar month from October 1940.
export function dateOf(gt: number): GameDate {
  if (gt === 1) return { year: 1940, month: 9, week: 3 };
  if (gt === 2) return { year: 1940, month: 9, week: 4 };
  const n = gt - 3;
  const week = (n % 4) + 1;
  const abs = 1940 * 12 + 9 + Math.floor(n / 4);
  return { year: Math.floor(abs / 12), month: (abs % 12) + 1, week };
}

const MON = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
export function monthLabel(gt: number): string {
  const d = dateOf(gt);
  return `${MON[d.month - 1]} ${d.year}`;
}
export function weekLabel(gt: number): string {
  const d = dateOf(gt);
  return `${MON[d.month - 1]} wk${d.week} · GT${gt}`;
}

/** The grouping key for one event at a given level (empty for "step" — every step is its own node). */
export function levelKey(ev: Event, level: Level): string {
  switch (level) {
    case "week":
      return `${ev.game_turn}`;
    case "stage":
      return `${ev.game_turn}.${ev.operations_stage}`;
    case "phase":
      return `${ev.game_turn}.${ev.operations_stage}.${ev.phase}`;
    case "step":
      return "";
  }
}

/** A short label for the node a step belongs to, at a given level. */
export function nodeLabel(ev: Event, level: Level): string {
  switch (level) {
    case "week":
      return weekLabel(ev.game_turn);
    case "stage":
      return `GT${ev.game_turn} · Stage ${ev.operations_stage}`;
    case "phase":
      return `GT${ev.game_turn} · S${ev.operations_stage} · ${ev.phase}`;
    case "step":
      return `step ${ev.step}`;
  }
}

export interface Node {
  start: number; // first event index of this node
  label: string;
}

/** The ordered nodes at a grouping level (month/week/stage/phase). Not used for "step". */
export function buildNodes(events: Event[], level: Level): Node[] {
  const out: Node[] = [];
  let prev: string | null = null;
  for (let i = 0; i < events.length; i++) {
    const k = levelKey(events[i], level);
    if (k !== prev) {
      out.push({ start: i, label: nodeLabel(events[i], level) });
      prev = k;
    }
  }
  return out;
}

/** Index of the node containing `step` (last node whose start ≤ step). */
export function nodeIndexOf(nodes: Node[], step: number): number {
  let lo = 0;
  for (let i = 0; i < nodes.length; i++) if (nodes[i].start <= step) lo = i;
  return lo;
}
