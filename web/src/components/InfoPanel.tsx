import type { InfoPanelData, InfoSection } from "../lore";

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

export function InfoPanel({ title, lore, rules }: InfoPanelData) {
  return (
    <>
      <div className="lore-title">{title}</div>
      {lore && <Section icon="book_5" section={lore} />}
      {lore && rules && <hr className="info-divider" />}
      {rules && <Section icon="gavel" section={rules} />}
    </>
  );
}
