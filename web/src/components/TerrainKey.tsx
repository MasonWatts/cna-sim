import { CREAM, HEX_STROKE, PATTERNS, edgeGlyph, featureGlyph, routeGlyph, terrainFill } from "./mapSymbols";
import { hexPolygon } from "../hex";
import type { InfoPanelData } from "../lore";

// Prototype hybrid entry: placeholder lore + rules for the Salt Marsh terrain (§8). Real content will
// be supplied later; this proves the reusable InfoPanel wiring from a terrain-key swatch.
const SALT_MARSH_INFO: InfoPanelData = {
  title: "Salt Marsh",
  lore: {
    description:
      "Salt marshes — the North African sebkha — are shallow depressions floored with fine, salt-crusted sand. A firm-looking surface often hides a quicksand-like bog that could swallow a vehicle to its axles. Their impassability shaped the campaign: the vast sabkha of the Qattara Depression anchored the southern flank at El Alamein, denying any sweep around the line and forcing a head-on fight.",
    links: [{ label: "Sabkha (Wikipedia)", url: "https://en.wikipedia.org/wiki/Sabkha" }],
  },
  rules: {
    description:
      "§8.44: treacherous salt sand — a grave for vehicles. Except Light Trucks, Recce-type units, and motorcycle infantry, vehicles may enter or leave a Salt Marsh only along a Road or Track; a prohibited vehicle that enters off-track is Abandoned (§5.33). No motorized unit or AFV may assault a defender in a Salt Marsh. Entry costs 2 CP motorized / 3 non-motorized, and a defender there shifts Close Assault one column in its favor (barrage and anti-armor are unaffected).",
  },
};

// A cosmetic legend mirroring the paper map's TERRAIN KEY, but drawn with our own patterns/glyphs.
// Grouped in the same order as the printed key. Symbols we don't render (Coast, Border, Airfield,
// Flying-Boat facilities, Training Area, Off-Map Airbases) are omitted.

const W = 54;
const H = 48;
const CX = W / 2;
const CY = H / 2;

// `h` lets thin symbols (routes/edges, which are just a line at the vertical centre) use a shorter
// swatch so the label sits close instead of below a tall empty box.
function Cell({ label, children, h = H, hover }: { label: string; children: React.ReactNode; h?: number; hover?: { onMouseEnter: (e: React.MouseEvent) => void; onMouseLeave: () => void } }) {
  return (
    <div
      style={{ display: "flex", flexDirection: "column", alignItems: "center", width: 66, cursor: hover ? "help" : undefined }}
      onMouseEnter={hover?.onMouseEnter}
      onMouseLeave={hover?.onMouseLeave}
    >
      <svg width={W} height={h} viewBox={`0 0 ${W} ${h}`} style={{ display: "block" }}>
        {children}
      </svg>
      <span className="tk-label">{label}</span>
    </div>
  );
}

const hexSwatch = (fill: string) => <polygon points={hexPolygon(CX, CY, 17)} fill={fill} stroke={HEX_STROKE} strokeWidth={1} />;

// [terrain-key, display label]
const TERRAINS: [string, string][] = [
  ["Clear", "Clear"],
  ["SaltMarsh", "Salt Marsh"],
  ["Desert", "Desert"],
  ["SandGravel", "Rock/Gravel"],
  ["Rough", "Rough"],
  ["Mountain", "Mountain"],
  ["HeavyVegetation", "Heavy Veg."],
  ["Delta", "Delta"],
  ["Swamp", "Swamp"],
  ["Sea", "Sea"],
  ["MajorCity", "Major City"],
];

// [feature-glyph key, display label]
const FEATURES: [string, string][] = [
  ["Port", "Port"],
  ["Village", "Village/Bir"],
  ["Oasis", "Oasis"],
  ["CwTrainingArea", "Train. (CW)"],
  ["AxisTrainingArea", "Train. (Axis)"],
];

const ROUTES: [string, string][] = [
  ["Road", "Road"],
  ["UnfinishedRoad", "Unfin. Road"],
  ["Track", "Track"],
  ["Railroad", "Railroad"],
  ["UnfinishedRailroad", "Unfin. Rail"],
];

const EDGES: [string, string][] = [
  ["Ridge", "Ridge"],
  ["Escarpment", "Escarpment"],
  ["Slope", "Slope"],
  ["Wadi", "Wadi"],
  ["MajorRiver", "Major River"],
  ["MinorRiver", "Minor River"],
];

const Row = ({ children }: { children: React.ReactNode }) => (
  <div style={{ display: "flex", flexWrap: "wrap", gap: 4 }}>{children}</div>
);

export function TerrainKey({ label = "Terrain Key", onInfoHover, onInfoLeave }: { label?: string; onInfoHover?: (data: InfoPanelData, x: number, y: number) => void; onInfoLeave?: () => void }) {
  return (
    <div className="terrain-key">
      <div className="tk-title">{label}</div>
      {/* pattern defs (self-contained so the key renders even if the map isn't mounted) */}
      <svg width={0} height={0} style={{ position: "absolute" }} aria-hidden>
        {PATTERNS}
      </svg>
      <div style={{ marginTop: 8, display: "flex", flexDirection: "column", gap: 10 }}>
        <Row>
          {TERRAINS.map(([k, lbl]) => {
            // Prototype: the Salt Marsh swatch opens the hybrid lore+rules panel on hover.
            const hover =
              k === "SaltMarsh" && onInfoHover
                ? {
                    onMouseEnter: (e: React.MouseEvent) => {
                      const r = e.currentTarget.getBoundingClientRect();
                      onInfoHover(SALT_MARSH_INFO, r.left, r.bottom);
                    },
                    onMouseLeave: () => onInfoLeave?.(),
                  }
                : undefined;
            return (
              <Cell key={k} label={lbl} hover={hover}>
                {hexSwatch(terrainFill(k))}
              </Cell>
            );
          })}
          {FEATURES.map(([k, lbl]) => {
            // Port/training glyphs carry a large map-hex offset (anchor −14, soldier +20) tuned for
            // the full board hex; in the smaller legend swatch that overshoots, so pull them back
            // toward center here (port down, training up a hair).
            const nudge = k === "Port" ? 11 : k.endsWith("TrainingArea") ? -10 : 0;
            return (
              <Cell key={k} label={lbl}>
                {hexSwatch(CREAM)}
                <g transform={`translate(${CX} ${CY + nudge})`}>{featureGlyph(k)}</g>
              </Cell>
            );
          })}
        </Row>
        <Row>
          {ROUTES.map(([k, lbl]) => (
            <Cell key={k} label={lbl} h={20}>
              <g transform={`translate(${CX} 10)`}>{routeGlyph(k, { x: -20, y: 0 }, { x: 20, y: 0 })}</g>
            </Cell>
          ))}
        </Row>
        <Row>
          {EDGES.map(([k, lbl]) => (
            <Cell key={k} label={lbl} h={36}>
              {/* A above / B below → the shared edge (and its glyph) runs horizontally */}
              <g transform={`translate(${CX} 18)`}>{edgeGlyph(k, { x: 0, y: -16 }, { x: 0, y: 16 }, true)}</g>
            </Cell>
          ))}
        </Row>
      </div>
    </div>
  );
}
