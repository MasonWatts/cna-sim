import { CREAM, HEX_STROKE, PATTERNS, edgeGlyph, featureGlyph, routeGlyph, terrainFill } from "./mapSymbols";
import { hexPolygon } from "../hex";

// A cosmetic legend mirroring the paper map's TERRAIN KEY, but drawn with our own patterns/glyphs.
// Grouped in the same order as the printed key. Symbols we don't render (Coast, Border, Airfield,
// Flying-Boat facilities, Training Area, Off-Map Airbases) are omitted.

const W = 54;
const H = 48;
const CX = W / 2;
const CY = H / 2;

function Cell({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <div style={{ display: "flex", flexDirection: "column", alignItems: "center", width: 66 }}>
      <svg width={W} height={H} viewBox={`0 0 ${W} ${H}`} style={{ display: "block" }}>
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
  ["TrainingArea", "Training Area"],
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

export function TerrainKey({ label = "Terrain Key" }: { label?: string }) {
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
            <Cell key={k} label={lbl}>
              {hexSwatch(terrainFill(k))}
            </Cell>
          ))}
          {FEATURES.map(([k, lbl]) => (
            <Cell key={k} label={lbl}>
              {hexSwatch(CREAM)}
              <g transform={`translate(${CX} ${CY})`}>{featureGlyph(k)}</g>
            </Cell>
          ))}
        </Row>
        <Row>
          {ROUTES.map(([k, lbl]) => (
            <Cell key={k} label={lbl}>
              <g transform={`translate(${CX} ${CY})`}>{routeGlyph(k, { x: -20, y: 0 }, { x: 20, y: 0 })}</g>
            </Cell>
          ))}
        </Row>
        <Row>
          {EDGES.map(([k, lbl]) => (
            <Cell key={k} label={lbl}>
              {/* A above / B below → the shared edge (and its glyph) runs horizontally */}
              <g transform={`translate(${CX} ${CY})`}>{edgeGlyph(k, { x: 0, y: -16 }, { x: 0, y: 16 }, true)}</g>
            </Cell>
          ))}
        </Row>
      </div>
    </div>
  );
}
