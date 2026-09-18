import { CREAM, HEX_STROKE, PATTERNS, edgeGlyph, featureGlyph, routeGlyph, terrainFill } from "./mapSymbols";
import { hexPolygon } from "../hex";
import type { InfoPanelData } from "../lore";
import { TERRAIN_INFO } from "../terrainInfo";

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

// A fixed 4-column grid so every row lines up uniformly (cells keep their own size).
const Row = ({ children }: { children: React.ReactNode }) => (
  <div style={{ display: "grid", gridTemplateColumns: "repeat(4, 66px)", gap: 4, justifyContent: "start" }}>{children}</div>
);

export function TerrainKey({ label = "Terrain Key", onInfoHover, onInfoLeave }: { label?: string; onInfoHover?: (data: InfoPanelData, x: number, y: number) => void; onInfoLeave?: () => void }) {
  // A legend swatch with an info entry opens the hybrid lore/rules panel on hover, anchored to the cell.
  const mkHover = (k: string) =>
    onInfoHover && TERRAIN_INFO[k]
      ? {
          onMouseEnter: (e: React.MouseEvent) => {
            const r = e.currentTarget.getBoundingClientRect();
            onInfoHover(TERRAIN_INFO[k], r.left, r.bottom);
          },
          onMouseLeave: () => onInfoLeave?.(),
        }
      : undefined;
  return (
    <div className="terrain-key">
      <div className="tk-title">{label}</div>
      {/* pattern defs (self-contained so the key renders even if the map isn't mounted) */}
      <svg width={0} height={0} style={{ position: "absolute" }} aria-hidden>
        {PATTERNS}
      </svg>
      <div style={{ marginTop: 8, display: "flex", flexDirection: "column", gap: 10 }}>
        <Row>
          {TERRAINS.map(([k, lbl]) => (
            <Cell key={k} label={lbl} hover={mkHover(k)}>
              {hexSwatch(terrainFill(k))}
            </Cell>
          ))}
          {FEATURES.map(([k, lbl]) => {
            // Port/training glyphs carry a large map-hex offset (anchor −14, soldier +20) tuned for
            // the full board hex; in the smaller legend swatch that overshoots, so pull them back
            // toward center here (port down, training up a hair).
            const nudge = k === "Port" ? 11 : k.endsWith("TrainingArea") ? -10 : 0;
            return (
              <Cell key={k} label={lbl} hover={mkHover(k)}>
                {hexSwatch(CREAM)}
                <g transform={`translate(${CX} ${CY + nudge})`}>{featureGlyph(k)}</g>
              </Cell>
            );
          })}
        </Row>
        <Row>
          {ROUTES.map(([k, lbl]) => (
            <Cell key={k} label={lbl} h={20} hover={mkHover(k)}>
              <g transform={`translate(${CX} 10)`}>{routeGlyph(k, { x: -20, y: 0 }, { x: 20, y: 0 })}</g>
            </Cell>
          ))}
          {EDGES.map(([k, lbl]) => (
            <Cell key={k} label={lbl} h={36} hover={mkHover(k)}>
              {/* A above / B below → the shared edge (and its glyph) runs horizontally */}
              <g transform={`translate(${CX} 18)`}>{edgeGlyph(k, { x: 0, y: -16 }, { x: 0, y: 16 }, true)}</g>
            </Cell>
          ))}
        </Row>
      </div>
    </div>
  );
}
