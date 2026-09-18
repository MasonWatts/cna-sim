import { unitTypeGlyph } from "./mapSymbols";

// A legend for the unit-type glyphs drawn on counters (see unitTypeGlyph). Each glyph is composed
// from a set of primitives (branch + modifiers) and shown on a small counter-style swatch (the
// glyphs are white, so they need a dark background like on the map).
//
// The set of types is **data-driven**: the recorder emits every distinct `unit_type` glyph present
// across the full Order of Battle (`GameRecord.unit_types`), so nothing that can appear on a counter
// falls through the cracks. This table only supplies friendly *labels* (and a preferred order);
// any OB type without a curated label still renders, labelled from its raw primitives.
// [primitive set, friendly label], ordered roughly by frequency.
const CURATED: [string[], string][] = [
  [["Infantry"], "Infantry"],
  [["Artillery"], "Artillery"],
  [["CoastalGun"], "Coastal defense"],
  [["Armor"], "Armor"],
  [["AntiAir"], "Anti-air"],
  [["AntiTank"], "Anti-tank"],
  [["Infantry", "MachineGun"], "Machine-gun"],
  [["Infantry", "Heavy", "Motorized"], "Motor heavy-wpn"],
  [["Infantry", "Armor"], "Support group"],
  [["Engineer"], "Engineer"],
  [["Infantry", "Bersaglieri"], "Bersaglieri"],
  [["Recon"], "Armored recon"],
  [["Hq"], "HQ"],
  [["ArmoredCar"], "Armored car"],
  [["Infantry", "Airborne"], "Parachute inf"],
  [["Infantry", "Airborne", "Heavy"], "Heavy para inf"],
  [["Cavalry"], "Cavalry"],
  [["Cavalry", "Camel"], "Camel cavalry"],
  [["Infantry", "Marine"], "Marine inf"],
  [["Tds"], "Tank delivery"],
  [["Recon", "Motorcycle"], "Motorcycle recon"],
  [["RailConstruction"], "RR construction"],
  [["Infantry", "Marine", "Motorized"], "Motor marine"],
  [["Infantry", "Commando"], "Commando"],
  [["TankDestroyer"], "Tank destroyer"],
  [["RoadConstruction"], "Road construction"],
  [["Infantry", "Motorized"], "Motorized inf"],
  [["AnyUnit"], "Generic unit"],
  [["SelfPropelled"], "Self-propelled"],
];

const key = (prims: string[]) => prims.join("+");
const LABEL = new Map(CURATED.map(([p, l]) => [key(p), l]));
const CURATED_ORDER = CURATED.map(([p]) => key(p));

// Fall back to a readable label composed from the raw primitive names (e.g. "Infantry Airborne Heavy").
function labelFor(prims: string[]): string {
  return LABEL.get(key(prims)) ?? prims.join(" ");
}

function Cell({ prims, label }: { prims: string[]; label: string }) {
  return (
    <div style={{ display: "flex", flexDirection: "column", alignItems: "center", width: 60 }}>
      <svg width={30} height={22} viewBox="0 0 30 22" style={{ display: "block" }}>
        <rect x={1} y={1} width={28} height={20} rx={3} fill="#5b6473" stroke="#0d0f14" strokeWidth={1} />
        <g transform="translate(15 11)">{unitTypeGlyph(prims)}</g>
      </svg>
      <span className="tk-label">{label}</span>
    </div>
  );
}

export function UnitKey({ label = "Unit Key", unitTypes }: { label?: string; unitTypes?: string[][] }) {
  // Data-driven when the recorder supplies the OB's glyph set; else fall back to the curated list.
  const sets = unitTypes && unitTypes.length > 0 ? unitTypes : CURATED.map(([p]) => p);
  // Show curated types first (in their preferred order), then any uncurated OB types.
  const ordered = [...sets].sort((a, b) => {
    const ia = CURATED_ORDER.indexOf(key(a));
    const ib = CURATED_ORDER.indexOf(key(b));
    return (ia === -1 ? 1e9 : ia) - (ib === -1 ? 1e9 : ib) || key(a).localeCompare(key(b));
  });
  return (
    <div className="terrain-key">
      <div className="tk-title">{label}</div>
      <div style={{ marginTop: 8, display: "grid", gridTemplateColumns: "repeat(4, 60px)", gap: 4, justifyContent: "start" }}>
        {ordered.map((prims) => (
          <Cell key={key(prims)} prims={prims} label={labelFor(prims)} />
        ))}
      </div>
    </div>
  );
}
