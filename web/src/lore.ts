// Historical-context ("lore") subsystem. Content lives in editable data files under
// `web/public/lore/` (fetched at runtime), NOT in viewer code — so a future content-authoring agent
// edits only the JSON, never the components. Places are the first kind; units/others follow the
// same shape (see `loadLore`).

export type LoreLink = { label: string; url: string };

export type LoreEntry = {
  /** Display heading; defaults to the entry's key (e.g. the place name). */
  title?: string;
  /** A few sentences of historical context. */
  description: string;
  /** Optional external "read more" links. */
  links?: LoreLink[];
  /** Optional rules note for this place (shown in the panel's gavel section, below the lore). */
  rules?: InfoSection;
};

/** A lore dataset keyed by the subject's name (place label / unit designation). */
export type LoreMap = Record<string, LoreEntry>;

/** One section of the hybrid info panel — a paragraph plus optional "read more" links. */
export type InfoSection = { description: string; links?: LoreLink[] };

/** One row of an element's dynamic-state readout (e.g. a depot's fuel/ammo levels). */
export type StackStateRow = { label: string; value: string };

/** Structured detail for one stacked-tile occupant, shown in the panel's detail pane. Up to three
 * sections, rendered top-to-bottom when present: general **rules** for the element type (gavel
 * icon), the element's dynamic **state** (person icon), and historical **lore** (book icon). A
 * plain `summary` is the fallback for element types not yet given structured detail. */
export type EntryDetail = {
  summary?: string;
  /** Optional counter glyph (a small inline SVG chip) shown at the top of the detail pane. */
  glyph?: import("react").ReactNode;
  rules?: InfoSection;
  state?: StackStateRow[];
  lore?: InfoSection;
};

/** One selectable occupant in a stacked-tile info panel: a short list label and its detail. */
export type StackEntry = { label: string; detail: EntryDetail };

/** Data for the reusable hybrid info panel: a title, an optional historical **lore** section (book
 * icon) and an optional **rules** section (gavel icon). Either or both may be present. An optional
 * pre-formatted `body` (newline-separated) renders verbatim above the sections. Optional `entries`
 * switch the panel into a master-detail layout — a scrollable list of counters on the left, the
 * selected counter's detail on the right — used for stacked map-tile tooltips. */
export type InfoPanelData = { title: string; body?: string; entries?: StackEntry[]; lore?: InfoSection; rules?: InfoSection };

/**
 * Fetch a lore dataset from `web/public/lore/<kind>.json` (e.g. `"places"`). Keys beginning with
 * `_` (like `_README`) are ignored. Missing/invalid files resolve to an empty map so the viewer
 * degrades gracefully — hovering just shows nothing.
 */
export async function loadLore(kind: string): Promise<LoreMap> {
  try {
    const res = await fetch(`/lore/${kind}.json`);
    if (!res.ok) return {};
    const raw = (await res.json()) as Record<string, unknown>;
    const out: LoreMap = {};
    for (const [key, val] of Object.entries(raw)) {
      if (key.startsWith("_")) continue;
      if (val && typeof val === "object" && typeof (val as LoreEntry).description === "string") {
        out[key] = val as LoreEntry;
      }
    }
    return out;
  } catch {
    return {};
  }
}
