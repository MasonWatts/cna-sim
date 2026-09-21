import { useEffect, useState, type ReactNode } from "react";
import type { EntryDetail, InfoPanelData, InfoSection, StackEntry, StackStateRow } from "../lore";

// Reusable hybrid info panel: a title, then an optional historical **lore** section (Material
// "book_5" icon, inline with the text) and an optional **rules** section (Material "gavel" icon),
// separated by a divider only when both are present. Content-agnostic — used for place labels,
// terrain-key entries, units, etc. (see InfoPanelData).

function Section({ icon, section }: { icon: string; section: InfoSection }) {
  return (
    <div className="info-section">
      <div className="info-body">
        <span className="material-symbols-outlined info-icon" aria-hidden="true">{icon}</span>
        <span>{section.description}</span>
      </div>
      {section.links && section.links.length > 0 && (
        <div className="lore-links">
          {section.links.map((l) => (
            <a key={l.url} href={l.url} target="_blank" rel="noopener noreferrer">
              {l.label} ↗
            </a>
          ))}
        </div>
      )}
    </div>
  );
}

// The element's dynamic state (current supply levels, strength, etc.) as a small labelled table,
// marked with a Material "person" icon to distinguish it from the rules (gavel) and lore (book).
function StateSection({ rows, glyph }: { rows: StackStateRow[]; glyph?: import("react").ReactNode }) {
  return (
    <div className="info-section">
      {glyph && <div className="state-glyph">{glyph}</div>}
      <div className="info-body">
        <span className="material-symbols-outlined info-icon" aria-hidden="true">person</span>
        <div className="state-rows">
          {rows.map((r) => (
            <div className="state-row" key={r.label}>
              <span className="state-label">{r.label}</span>
              <span className="state-value">{r.value}</span>
            </div>
          ))}
        </div>
      </div>
    </div>
  );
}

// Above this many state rows, a counter's detail is laid out in two columns (rules/lore stacked on
// the left, the tall state readout in its own column on the right) so the panel doesn't get very
// tall. Below it, everything stays in a single vertical stack.
const WIDE_STATE_ROWS = 8;

// The right-hand detail pane for the selected counter: up to a rules blurb, a dynamic-state
// readout, and lore — falling back to the plain summary line for not-yet-expanded types.
function DetailView({ detail: d }: { detail: EntryDetail }) {
  const stateRows = d.state && d.state.length > 0 ? d.state : null;
  const rulesLore = [
    d.rules && <Section key="rules" icon="gavel" section={d.rules} />,
    d.lore && <Section key="lore" icon="book_5" section={d.lore} />,
  ].filter(Boolean);
  const stack = (nodes: ReactNode[]) =>
    nodes.map((s, i) => (
      <div key={i}>
        {i > 0 && <hr className="info-divider" />}
        {s}
      </div>
    ));

  // Tall state + something to sit beside it → two columns.
  if (stateRows && stateRows.length > WIDE_STATE_ROWS && rulesLore.length > 0) {
    return (
      <div className="sl-detail sl-detail-wide">
        <div className="sl-detail-col">{stack(rulesLore)}</div>
        <div className="sl-col-divider" />
        <StateSection rows={stateRows} glyph={d.glyph} />
      </div>
    );
  }

  // Single vertical stack (rules → state → lore), or the plain summary fallback.
  const sections = [
    d.rules && <Section key="rules" icon="gavel" section={d.rules} />,
    stateRows && <StateSection key="state" rows={stateRows} glyph={d.glyph} />,
    d.lore && <Section key="lore" icon="book_5" section={d.lore} />,
    d.summary && !d.rules && !stateRows && !d.lore && <div key="summary" className="sl-summary">{d.summary}</div>,
  ].filter(Boolean);
  return <div className="sl-detail">{stack(sections)}</div>;
}

// Master-detail view for a stacked hex: a scrollable list of counters (left) and the selected
// counter's detail (right). Selection resets to the first entry whenever the stack changes.
function StackList({ entries }: { entries: StackEntry[] }) {
  const [sel, setSel] = useState(0);
  useEffect(() => setSel(0), [entries]);
  const active = entries[Math.min(sel, entries.length - 1)];
  return (
    <div className="info-stacklist">
      <div className="sl-list">
        {entries.map((e, i) => (
          <button
            key={i}
            className={`sl-item${i === sel ? " active" : ""}`}
            title={e.label}
            onClick={() => setSel(i)}
            onMouseEnter={() => setSel(i)}
          >
            {e.label}
          </button>
        ))}
      </div>
      <div className="sl-divider" />
      {active && <DetailView detail={active.detail} />}
    </div>
  );
}

export function InfoPanel({ title, body, entries, lore, rules }: InfoPanelData) {
  return (
    <>
      <div className="lore-title">{title}</div>
      {entries && entries.length > 0 && <StackList entries={entries} />}
      {body && <div className="info-stack">{body}</div>}
      {lore && <Section icon="book_5" section={lore} />}
      {lore && rules && <hr className="info-divider" />}
      {rules && <Section icon="gavel" section={rules} />}
    </>
  );
}
