// Paper-map palette (parchment base). Solid fills for flat terrains; the rest reference an SVG
// <pattern> (see PATTERNS) so textured terrains match the printed key.
export const CREAM = "#efe8c6";
const TERRAIN_FILL: Record<string, string> = {
  Clear: CREAM,
  Sea: "url(#patSea)",
  Desert: "url(#patDesert)",
  Rough: "#8f7f42",
  Delta: "#a7ad4e",
  Mountain: "url(#patMountain)",
  SandGravel: "url(#patGravel)",
  SaltMarsh: "url(#patSaltMarsh)",
  HeavyVegetation: "url(#patVeg)",
  Swamp: "url(#patSwamp)",
  MajorCity: "url(#patCity)",
};
export const terrainFill = (t: string) => TERRAIN_FILL[t] ?? CREAM;
export const HEX_STROKE = "#9c9066"; // parchment-gray hexside, like the paper map
const FEATURE_BLUE = "#3f8fc4"; // the paper legend's feature blue (Oasis palms, Port anchor, dots)

export const ROUTE_DARK = "#4a4644";
const RAIL_BLUE = "#2f9e86"; // teal-green, per the paper map — distinct from the river blue
export const ROUTE_KINDS = new Set(["Road", "UnfinishedRoad", "Track", "Railroad", "UnfinishedRailroad", "DestroyedRailroad"]);
export const EDGE_KINDS = new Set(["MajorRiver", "MinorRiver", "Wadi", "Ridge", "Escarpment", "Slope"]);
const EDGE_HALF = 13; // half a hex side (circumradius/2) — the shared edge's half-length

/** An edge-terrain feature drawn ON the shared edge between adjacent hexes A and B (the segment
 *  between their two shared vertices). Escarpment/Slope hachure toward the high side (`highIsB`). */
export function edgeGlyph(kind: string, A: { x: number; y: number }, B: { x: number; y: number }, highIsB: boolean | null): JSX.Element {
  const dx = B.x - A.x;
  const dy = B.y - A.y;
  const len = Math.hypot(dx, dy) || 1;
  const ux = dx / len; // along the center line (toward B)
  const uy = dy / len;
  const ex = -uy; // along the edge (perpendicular to center line)
  const ey = ux;
  const mx = (A.x + B.x) / 2;
  const my = (A.y + B.y) / 2;
  const p1 = { x: mx + ex * EDGE_HALF, y: my + ey * EDGE_HALF };
  const p2 = { x: mx - ex * EDGE_HALF, y: my - ey * EDGE_HALF };
  const edge = (stroke: string, w: number, opacity = 1) => (
    <line x1={p1.x} y1={p1.y} x2={p2.x} y2={p2.y} stroke={stroke} strokeWidth={w} strokeLinecap="round" opacity={opacity} />
  );

  switch (kind) {
    case "MajorRiver": {
      // A darker-blue bank showing as a thin border along the sides, with the lighter channel on
      // top. The bank uses a butt cap (no rounded end) so it doesn't blob past the segment ends
      // where consecutive river segments join.
      return (
        <g>
          <line x1={p1.x} y1={p1.y} x2={p2.x} y2={p2.y} stroke="#2f6f9e" strokeWidth={3.4} strokeLinecap="butt" />
          {edge("#4a9ed6", 2.2)}
        </g>
      );
    }
    case "MinorRiver":
      // Same channel blue as MajorRiver but no darker bank — the bank is what distinguishes them.
      return edge("#4a9ed6", 1.8);
    case "Wadi":
      return edge("#6fae5a", 2.8, 0.75);
    case "Ridge":
      return edge("#8a6f3a", 2.2);
    case "Escarpment":
    case "Slope": {
      const esc = kind === "Escarpment";
      const color = esc ? "#6a6a6a" : "#9a8a5a";
      // Hachures point from the edge toward the low hex (down-slope, away from the high side).
      const sign = highIsB === false ? 1 : -1; // toward the low hex (opposite the high side)
      const L = esc ? 5 : 3.5;
      const hx = ux * sign * L;
      const hy = uy * sign * L;
      const ticks = [0.16, 0.38, 0.62, 0.84].map((t, i) => {
        const bx = p1.x + (p2.x - p1.x) * t;
        const by = p1.y + (p2.y - p1.y) * t;
        return <line key={i} x1={bx} y1={by} x2={bx + hx} y2={by + hy} stroke={color} strokeWidth={esc ? 1 : 0.8} strokeLinecap="round" />;
      });
      return (
        <g>
          {edge(color, esc ? 1.6 : 1.3)}
          {ticks}
        </g>
      );
    }
    default:
      return edge("#888", 1);
  }
}

/** A linear route (road/track/rail) drawn center-to-center across the shared edge of two adjacent
 *  hexes A and B. Roads are double lines, tracks a single dashed line, rails a line with cross-ties;
 *  the "Unfinished" variants dash the line. */
export function routeGlyph(kind: string, A: { x: number; y: number }, B: { x: number; y: number }): JSX.Element {
  const dx = B.x - A.x;
  const dy = B.y - A.y;
  const len = Math.hypot(dx, dy) || 1;
  const nx = -dy / len; // unit perpendicular
  const ny = dx / len;

  if (kind === "Track") {
    return (
      <line x1={A.x} y1={A.y} x2={B.x} y2={B.y} stroke={ROUTE_DARK} strokeWidth={1.1} strokeLinecap="round" strokeDasharray="3 2.5" />
    );
  }
  if (kind === "Road" || kind === "UnfinishedRoad") {
    const o = 1.4;
    const dash = kind === "UnfinishedRoad" ? "3 2" : undefined;
    return (
      <g stroke={ROUTE_DARK} strokeWidth={1.1} strokeDasharray={dash}>
        <line x1={A.x + nx * o} y1={A.y + ny * o} x2={B.x + nx * o} y2={B.y + ny * o} />
        <line x1={A.x - nx * o} y1={A.y - ny * o} x2={B.x - nx * o} y2={B.y - ny * o} />
      </g>
    );
  }
  if (kind === "DestroyedRailroad") {
    // §24.66 blown rail: a grey spine the same thickness as a live railroad, with small ✕ marks
    // where a live line has orthogonal ties.
    const GREY = "#9a9a9a";
    const XS = 1.7; // half-diagonal of each ✕
    const ux = dx / len;
    const uy = dy / len;
    const N = 6;
    const marks = [];
    for (let i = 0; i <= N; i++) {
      const t = i / N;
      const px = A.x + dx * t;
      const py = A.y + dy * t;
      const a1x = (ux + nx) * XS * 0.71;
      const a1y = (uy + ny) * XS * 0.71;
      const a2x = (ux - nx) * XS * 0.71;
      const a2y = (uy - ny) * XS * 0.71;
      marks.push(<line key={`a${i}`} x1={px - a1x} y1={py - a1y} x2={px + a1x} y2={py + a1y} strokeWidth={1.1} />);
      marks.push(<line key={`b${i}`} x1={px - a2x} y1={py - a2y} x2={px + a2x} y2={py + a2y} strokeWidth={1.1} />);
    }
    return (
      <g stroke={GREY} strokeLinecap="round">
        <line x1={A.x} y1={A.y} x2={B.x} y2={B.y} strokeWidth={1.3} />
        {marks}
      </g>
    );
  }
  // Railroad / UnfinishedRailroad: a line with perpendicular cross-ties. Ties are placed at the
  // segment endpoints too (i = 0..n): adjacent segments meet at a hex center, so their end ties
  // coincide there — keeping the spacing uniform across joins instead of a double gap.
  const dashed = kind === "UnfinishedRailroad";
  // Fixed n=6 subdivision so ties fall at t = 0, 1/6 … 3/6 (the edge) … 6/6 (the next center).
  // Ties at both centers coincide with the neighbour segment's, so runs dovetail; two ties sit
  // evenly between each center and the edge. Uniform spacing, independent of segment length.
  const TIE = 2.5; // half-length of each cross-tie (the vertical arm of the plus)
  const N = 6;
  const ties = [];
  for (let i = 0; i <= N; i++) {
    const t = i / N;
    const px = A.x + dx * t;
    const py = A.y + dy * t;
    ties.push(<line key={i} x1={px + nx * TIE} y1={py + ny * TIE} x2={px - nx * TIE} y2={py - ny * TIE} strokeWidth={1.2} />);
  }
  // Unfinished rail: identical ties, but the spine is gapped — dash sized to the tie interval
  // (len/N) so a gap falls between each pair of ties.
  const seg = len / N;
  return (
    <g stroke={RAIL_BLUE} strokeLinecap="round">
      <line
        x1={A.x}
        y1={A.y}
        x2={B.x}
        y2={B.y}
        strokeWidth={1.3}
        strokeLinecap={dashed ? "butt" : "round"}
        strokeDasharray={dashed ? `${TIE * 2} ${Math.max(0.5, seg - TIE * 2)}` : undefined}
        strokeDashoffset={dashed ? TIE : undefined}
      />
      {ties}
    </g>
  );
}

/** Point-feature glyph centered at (0,0) — the caller translates it to the hex center.
 *  Oasis (palms on a pool) and Port (anchor in a ring) mimic the paper legend; Bir and Village are
 *  a small filled dot for now (text labels to differentiate/identify come later). */
// Top-down plane silhouette per SquadronClass (§35.21), recreating the rulebook's air-unit art:
// Fighter = sleek single-engine with swept, tapered wings and no nacelles; Bomber = wide straight
// wings with four engine nacelles and a broad cruciform tail; Transport = wide wings, a bulkier
// fuselage, and two engine nacelles. Nose points up; drawn to fit roughly a ±6 × ±5 box. The caller
// supplies fill/stroke (a light silhouette on the counter). Unknown classes fall back to Fighter.
// Side-profile warship silhouette by type, keyed off the §30.12 Gun Rating printed on the counter
// (6 = BB, 4 = CA, 3 = CL, 2 = CLAA, 1 = DD): hull length + main-gun turrets scale with the rating;
// the AA cruiser shows AA mounts instead of turrets; the wreck (San Giorgio) lists and is broken.
// Caller supplies fill/stroke. Drawn to fit roughly a ±7 × ±4 box.
export function shipGlyph(gun: number, wreck: boolean): JSX.Element {
  if (wreck) {
    // A half-sunk, listing hull broken amidships (San Giorgio).
    return (
      <g transform="rotate(-8)">
        <path d="M-6,0 L2.2,0 L3.6,2.4 L-5,2.4 Z" />
        <path d="M3.2,0 L6,0 L5.2,2.4 L4.2,2.4 Z" />
        <rect x={-2.2} y={-1.6} width={1.4} height={1.8} />
      </g>
    );
  }
  // Match the counter art: a low side-profile recognition silhouette — a long flat hull with a
  // jagged superstructure — scaling from a long, busy battleship (Gun 6) to a small destroyer
  // (Gun 1). Heavy/light/AA cruisers (4/3/2) share one cruiser profile, as on the counters.
  if (gun >= 6) {
    // Battleship: long hull, tall central bridge tower, fore/aft turret and funnel bumps.
    return (
      <g>
        <path d="M-7,1 L7,1 L6,2.4 L-6,2.4 Z" />
        <path d="M-5.6,1 L-5.3,0.2 L-4,0.2 L-3.8,-0.6 L-2.6,-0.6 L-2.4,-2.4 L-1.6,-2.4 L-1.4,-0.6 L0.4,-0.6 L0.6,-1.5 L1.4,-1.5 L1.6,-0.6 L3.2,-0.6 L3.4,0.2 L4.9,0.2 L5.1,1 Z" />
      </g>
    );
  }
  if (gun >= 2) {
    // Cruiser (CA/CL/CLAA): medium hull, central bridge, a couple of low bumps.
    return (
      <g>
        <path d="M-5,1 L5,1 L4.2,2.4 L-4.2,2.4 Z" />
        <path d="M-3.6,1 L-3.3,0.1 L-1.7,0.1 L-1.5,-1.6 L-0.7,-1.6 L-0.5,0.1 L1.8,0.1 L2,-0.7 L2.8,-0.7 L3,1 Z" />
      </g>
    );
  }
  // Destroyer: short low hull, a small bridge and a funnel.
  return (
    <g>
      <path d="M-3.4,1 L3.4,1 L2.7,2.4 L-2.7,2.4 Z" />
      <path d="M-1.9,1 L-1.7,-0.5 L-0.9,-0.5 L-0.7,1 L0.5,1 L0.7,-1.2 L1.2,-1.2 L1.4,1 Z" />
    </g>
  );
}

export function squadronGlyph(cls: string): JSX.Element {
  if (cls === "Bomber") {
    return (
      <g>
        {/* long fuselage */}
        <path d="M0,-5 L0.9,-4 L0.9,4 L0,5 L-0.9,4 L-0.9,-4 Z" />
        {/* wide, near-straight wings */}
        <path d="M-6.2,0.6 L-1,-0.4 L1,-0.4 L6.2,0.6 L6.2,1.7 L-6.2,1.7 Z" />
        {/* broad tailplane */}
        <path d="M-3.2,3.3 L3.2,3.3 L3.2,4.4 L-3.2,4.4 Z" />
        {/* four engine nacelles on the wing leading edge */}
        <rect x={-4.6} y={-1.1} width={1.1} height={1.9} rx={0.3} />
        <rect x={-2.3} y={-1.3} width={1.1} height={2.0} rx={0.3} />
        <rect x={1.2} y={-1.3} width={1.1} height={2.0} rx={0.3} />
        <rect x={3.5} y={-1.1} width={1.1} height={1.9} rx={0.3} />
      </g>
    );
  }
  if (cls === "Transport") {
    return (
      <g>
        {/* bulkier fuselage */}
        <path d="M0,-4.6 L1.3,-3.2 L1.3,2.4 L1,4 L-1,4 L-1.3,2.4 L-1.3,-3.2 Z" />
        {/* wide wings */}
        <path d="M-6,0.5 L-1,-0.4 L1,-0.4 L6,0.5 L6,1.6 L-6,1.6 Z" />
        {/* tailplane */}
        <path d="M-2.9,3 L2.9,3 L2.9,4 L-2.9,4 Z" />
        {/* two engine nacelles */}
        <rect x={-3.2} y={-1.1} width={1.2} height={2.0} rx={0.3} />
        <rect x={2.0} y={-1.1} width={1.2} height={2.0} rx={0.3} />
      </g>
    );
  }
  // Fighter (default): sleek single-engine, swept tapered wings, small cruciform tail, no nacelles.
  return (
    <path d="M0,-5 L0.7,-3.2 L0.7,-0.8 L5,0.7 L0.9,1.5 L0.8,3.1 L2.4,4.1 L0.5,4.6 L0,5 L-0.5,4.6 L-2.4,4.1 L-0.8,3.1 L-0.9,1.5 L-5,0.7 L-0.7,-0.8 L-0.7,-3.2 Z" />
  );
}

/** The §36 air-facility map symbols, copied from the paper map: a right triangle (Airfield / air
 *  landing strip); an anchor beside that triangle (Flying Boat Basin); an anchor beside two crossed
 *  strokes (Flying Boat Alighting Area). Drawn as light strokes centered at the origin (~12px box) so
 *  it reads on the slate counter; `kind` is the `AirFacilityView.kind` string. */
export function airfieldGlyph(kind: string): JSX.Element {
  const S = { stroke: "#e9e4cf", strokeWidth: 1.2, fill: "none", strokeLinecap: "round" as const, strokeLinejoin: "round" as const };
  // Right triangle: short left vertical, horizontal base, hypotenuse sloping to a point at the right.
  const triangle = (ox: number, s: number) => (
    <path d={`M${ox - 5 * s},${-4 * s} L${ox - 5 * s},${3 * s} L${ox + 6 * s},${3 * s} Z`} />
  );
  // The alighting-area lane: the Airfield triangle mirrored horizontally with the (now right-hand)
  // vertical edge removed — i.e. its base plus its hypotenuse (which now slopes up to the right).
  const crossed = (ox: number, s: number) => (
    <>
      <line x1={ox - 6 * s} y1={3 * s} x2={ox + 5 * s} y2={-4 * s} />
      <line x1={ox - 6 * s} y1={3 * s} x2={ox + 5 * s} y2={3 * s} />
    </>
  );
  // A small anchor: ring, shank, stock crossbar, and a shallow fluke arc.
  const anchor = (ox: number) => (
    <>
      <circle cx={ox} cy={-3.2} r={0.75} />
      <line x1={ox} y1={-2.5} x2={ox} y2={2.2} />
      <line x1={ox - 1.3} y1={-1.3} x2={ox + 1.3} y2={-1.3} />
      <path d={`M${ox - 1.7},0.6 Q${ox},2.8 ${ox + 1.7},0.6`} />
    </>
  );
  if (kind === "FlyingBoatBasin") return <g {...S}>{anchor(-5)}{triangle(1.5, 0.72)}</g>;
  if (kind === "FlyingBoatAlightingArea") return <g {...S}>{anchor(-5)}{crossed(1.5, 0.72)}</g>;
  return <g {...S}>{triangle(0, 1)}</g>; // Airfield / AirLandingStrip
}

export function featureGlyph(feature: string): JSX.Element | null {
  switch (feature) {
    case "Bir":
    case "Village":
      return <circle r={3.5} fill={FEATURE_BLUE} />;
    case "CwTrainingArea":
    case "AxisTrainingArea": {
      // §17.32/§20.43 training-ground marker: a standing soldier holding a grounded rifle (the paper
      // map shows a row of them; one here for space). Side-colored — CW blue, Axis red — since the
      // two areas are not interchangeable. Drawn with a cream halo so it reads on the city pattern.
      const color = feature === "AxisTrainingArea" ? "#b5533f" : FEATURE_BLUE;
      const body = (
        <>
          <circle cx={0} cy={-11} r={2.1} />
          <path d="M-2.4,-9.5 L2.4,-9.5 L1.7,-4.5 L-1.7,-4.5 Z" />
          <path d="M-2.2,-9 L-5.6,-6.8 L-5,-5.9 L-1.9,-7.8 Z" />
          <path d="M2.2,-9 L3.8,-6.4 L2.9,-6.1 L1.8,-7.8 Z" />
          <path d="M-1.7,-4.5 L-2.3,2 L-0.6,2 L-0.2,-4.5 Z" />
          <path d="M0.2,-4.5 L0.6,2 L2.3,2 L1.7,-4.5 Z" />
        </>
      );
      return (
        // Offset to the bottom of the hex, mirroring the Port anchor's top placement. The anchor's
        // visual center sits ~14 above the hex center; the soldier's body sits above its own origin,
        // so +20 puts its center ~14 below center — the symmetric bottom position.
        <g transform="translate(0 20)">
          {/* rifle: vertical, butt on the ground to the soldier's left */}
          <line x1={-5.8} y1={-8.6} x2={-5.8} y2={2.2} stroke="#f4eecb" strokeWidth={2.4} strokeLinecap="round" />
          <g fill={color} stroke="#f4eecb" strokeWidth={1.5} paintOrder="stroke" strokeLinejoin="round">
            {body}
          </g>
          <line x1={-5.8} y1={-8.6} x2={-5.8} y2={2.2} stroke={color} strokeWidth={1} strokeLinecap="round" />
        </g>
      );
    }
    case "Port": {
      // Offset toward the top of the hex (the sea side) so the anchor clears a center Village dot.
      // Drawn twice: a thicker white halo under the blue so it reads on the MajorCity block pattern.
      const anchor = (
        <>
          <circle r={8} />
          <circle cx={0} cy={-5} r={1.3} />
          <line x1={0} y1={-3.6} x2={0} y2={4.6} />
          <line x1={-3} y1={-1.6} x2={3} y2={-1.6} />
          <path d="M-4,2 Q0,6.6 4,2" />
        </>
      );
      return (
        <g transform="translate(0 -14)" fill="none" strokeLinecap="round" strokeLinejoin="round">
          <g stroke="#ffffff" strokeWidth={3} opacity={0.9}>{anchor}</g>
          <g stroke={FEATURE_BLUE} strokeWidth={1.3}>{anchor}</g>
        </g>
      );
    }
    case "Oasis":
      return (
        <>
          <ellipse cx={0} cy={6} rx={8} ry={2.2} fill={FEATURE_BLUE} opacity={0.5} />
          <g stroke={FEATURE_BLUE} strokeWidth={1.2} fill="none" strokeLinecap="round">
            <path d="M-3,6 Q-4,0 -3,-5" />
            <path d="M-3,-5 L-7,-7 M-3,-5 L-6,-3 M-3,-5 L1,-7 M-3,-5 L0,-3" />
            <path d="M4,6 Q3,-1 4,-7" />
            <path d="M4,-7 L0,-9 M4,-7 L1,-5 M4,-7 L8,-9 M4,-7 L7,-5" />
          </g>
        </>
      );
    default:
      return null;
  }
}

// Terrain <pattern> definitions, tiled in user (map) space so they zoom with the board. Rendered
// once in the SVG <defs>.
// Compose a counter glyph from a set of primitives (branch symbols + overlaid modifiers), per
// docs/unit-glyphs.md. Drawn white, centred at the origin (the caller positions it on the TOE line).
export function unitTypeGlyph(primitives: string[]): JSX.Element | null {
  if (!primitives || primitives.length === 0) return null;
  const S = "#fff";
  const sw = 1.0;
  const has = (p: string) => primitives.includes(p);
  const parts: JSX.Element[] = [];
  const add = (el: JSX.Element) => parts.push(<g key={parts.length}>{el}</g>);

  // NATO-style unit frame: a rectangle bounding box around the symbol. Mobility wheels hang below it.
  const FW = 6.2; // half-width
  const FH = 4.2; // half-height
  const frame = <rect key="frame" x={-FW} y={-FH} width={2 * FW} height={2 * FH} fill="none" stroke={S} strokeWidth={sw} />;

  // Standalone roles (label centred in the frame).
  const role = (t: string, size: number) => (
    <g>{frame}<text y={size / 2 - 0.6} textAnchor="middle" fontSize={size} fontWeight={700} fill={S} fontFamily="sans-serif">{t}</text></g>
  );
  if (has("Hq")) return role("HQ", 7);
  if (has("Tds")) return role("TDS", 5.6);
  if (has("RailConstruction")) return role("RR", 6.5);
  // Road construction: two crossed hammers/picks, tool heads at the lower (down) ends of the handles.
  if (has("RoadConstruction"))
    return (
      <g fill="none" stroke={S} strokeWidth={sw} strokeLinecap="round" strokeLinejoin="round">
        {frame}
        <line x1={3} y1={-2.8} x2={-3} y2={2.8} />
        <line x1={-3} y1={-2.8} x2={3} y2={2.8} />
        {/* blocky tool heads at the lower ends */}
        <g strokeWidth={1.9} strokeLinecap="butt">
          <line x1={-3.7} y1={2.0} x2={-2.3} y2={3.6} />
          <line x1={3.7} y1={2.0} x2={2.3} y2={3.6} />
        </g>
      </g>
    );

  const heavy = has("Heavy");
  const sp = has("SelfPropelled") || (has("Armor") && (has("Artillery") || has("AntiTank")));
  // The infantry ✕ fills the frame (⊠); its corners sit just inside the border.
  const xw = FW - 0.5;
  const xh = FH - 0.4;

  // Self-propelled / plain armor / support group (Infantry+Armor): the armor "track" — a stadium
  // (rectangle with fully rounded ends), behind any co-drawn branch.
  if (sp || has("Armor")) {
    add(<rect x={-4.2} y={-2.4} width={8.4} height={4.8} rx={2.4} ry={2.4} fill="none" stroke={S} strokeWidth={sw} />);
  }
  // Infantry / machine-gun ✕ (Commando draws its own smaller ✕ below).
  if ((has("Infantry") || has("MachineGun")) && !has("Commando")) {
    add(
      <g fill="none" stroke={S} strokeWidth={sw} strokeLinecap="round">
        <line x1={-xw} y1={-xh} x2={xw} y2={xh} />
        <line x1={-xw} y1={xh} x2={xw} y2={-xh} />
      </g>,
    );
    if (has("MachineGun")) {
      // A small vertical (up) arrow over the centre of the ✕.
      add(
        <g fill="none" stroke={S} strokeWidth={sw} strokeLinecap="round" strokeLinejoin="round">
          <line x1={0} y1={2.2} x2={0} y2={-2.4} />
          <polyline points="-1.3,-1 0,-2.8 1.3,-1" />
        </g>,
      );
    }
  }
  if (has("Artillery")) add(<circle cx={0} cy={0} r={1.7} fill={S} />);
  // Coastal-defense gun: a diamond through the midpoints of the box edges (top/bottom/left/right),
  // with an artillery-sized centre dot.
  if (has("CoastalGun"))
    add(
      <g stroke={S} strokeWidth={sw}>
        <polygon points={`0,${-xh} ${xw},0 0,${xh} ${-xw},0`} fill="none" strokeLinejoin="round" />
        <circle cx={0} cy={0} r={1.7} fill={S} stroke="none" />
      </g>,
    );
  // Anti-tank: arms from the two bottom corners up to the midpoint of the top edge.
  if (has("AntiTank")) add(<polyline points={`${-xw},${xh} 0,${-xh} ${xw},${xh}`} fill="none" stroke={S} strokeWidth={sw} strokeLinejoin="round" />);
  // Tank destroyer (SP anti-tank): AT chevron + armor stadium + artillery dot.
  if (has("TankDestroyer"))
    add(
      <g fill="none" stroke={S} strokeWidth={sw} strokeLinejoin="round">
        <rect x={-4.2} y={-2.4} width={8.4} height={4.8} rx={2.4} ry={2.4} />
        <polyline points={`${-xw},${xh} 0,${-xh} ${xw},${xh}`} />
        <circle cx={0} cy={0} r={1.25} fill={S} stroke="none" />
      </g>,
    );
  // Anti-air: the anti-tank chevron with a horizontal crossbar → an "A" (filled top when heavy).
  if (has("AntiAir")) {
    const abar = xh * 0.4; // crossbar height
    const ax = (xw * (abar + xh)) / (2 * xh); // arm x at the crossbar
    add(
      <g fill="none" stroke={S} strokeWidth={sw} strokeLinejoin="round">
        <polyline points={`${-xw},${xh} 0,${-xh} ${xw},${xh}`} />
        <line x1={-ax} y1={abar} x2={ax} y2={abar} />
        {heavy && <polygon points={`${-ax},${abar} 0,${-xh} ${ax},${abar}`} fill={S} stroke="none" />}
      </g>,
    );
  }
  if (has("Cavalry")) add(<line x1={-xw} y1={xh} x2={xw} y2={-xh} stroke={S} strokeWidth={sw} strokeLinecap="round" />);
  // Armored car: the armor stadium (car body), raised, sitting on three wheels.
  if (has("ArmoredCar"))
    add(
      <g stroke={S} strokeWidth={sw}>
        <rect x={-4} y={-3} width={8} height={4} rx={2} ry={2} fill="none" />
        <circle cx={-2.4} cy={2.6} r={0.95} fill={S} stroke="none" />
        <circle cx={0} cy={2.6} r={0.95} fill={S} stroke="none" />
        <circle cx={2.4} cy={2.6} r={0.95} fill={S} stroke="none" />
      </g>,
    );
  // Armored recon: the armor stadium with a diagonal slash. Motorcycle recce is not armored — just
  // the slash (the stadium is dropped; motorcycle wheels are added below).
  if (has("Recon"))
    add(
      <g fill="none" stroke={S} strokeWidth={sw}>
        {!has("Motorcycle") && <rect x={-4.2} y={-2.4} width={8.4} height={4.8} rx={2.4} ry={2.4} />}
        <line x1={-xw} y1={xh} x2={xw} y2={-xh} />
      </g>,
    );
  if (has("Engineer"))
    add(
      <g fill="none" stroke={S} strokeWidth={sw} strokeLinecap="round" strokeLinejoin="round">
        <line x1={-3.2} y1={-1.8} x2={3.2} y2={-1.8} />
        <line x1={-3.2} y1={-1.8} x2={-3.2} y2={1.8} />
        <line x1={0} y1={-1.8} x2={0} y2={1.8} />
        <line x1={3.2} y1={-1.8} x2={3.2} y2={1.8} />
      </g>,
    );

  // "Any unit" with no branch → the empty frame box.
  if (parts.length === 0 && has("AnyUnit")) return <g>{frame}</g>;

  // Modifiers overlaid on the branch.
  // Motorized (and Bersaglieri, which is drawn as motorized): wheels just above where the ✕ arms meet
  // the bottom edge, tucked inside the lower corners.
  if (has("Motorized") || has("Bersaglieri")) {
    add(<circle cx={-xw + 0.9} cy={xh - 2.4} r={1.1} fill={S} />);
    add(<circle cx={xw - 0.9} cy={xh - 2.4} r={1.1} fill={S} />);
  }
  // Motorcycle: with Recon, two rings under the right side of the slash; on infantry (e.g. Bersaglieri
  // motorcycle infantry), two wheels under the ✕ intersection.
  if (has("Motorcycle")) {
    if (has("Recon")) {
      add(<circle cx={1.4} cy={2.6} r={1.5} fill="none" stroke={S} strokeWidth={sw} />);
      add(<circle cx={4.6} cy={2.6} r={1.5} fill="none" stroke={S} strokeWidth={sw} />);
    } else {
      add(<circle cx={-1.3} cy={2.4} r={1.0} fill={S} />);
      add(<circle cx={1.3} cy={2.4} r={1.0} fill={S} />);
    }
  }
  // Airborne (parachute): a small gull just below the ✕ intersection.
  if (has("Airborne")) add(<path d="M -1.7 3.1 A 1.2 1.2 0 0 1 0 3.1 A 1.2 1.2 0 0 1 1.7 3.1" fill="none" stroke={S} strokeWidth={sw} strokeLinecap="round" />);
  // Heavy-weapons infantry (foot / motorized / parachute): the left portion of the box filled solid —
  // the same solid bar for all, so heavy-para matches heavy-inf and motor heavy-weapons.
  if (has("Infantry") && has("Heavy")) add(<rect x={-xw - 0.1} y={-xh} width={xw - 0.9} height={2 * xh} fill={S} stroke="none" />);
  if (has("Camel")) add(<text x={3.0} y={3.9} textAnchor="middle" fontSize={6.2} fontWeight={700} fill={S} fontFamily="sans-serif">c</text>);
  // Marine: an anchor overlaid on the infantry ✕ (ring, shaft, stock, and fluke arc).
  if (has("Marine"))
    add(
      <g fill="none" stroke={S} strokeWidth={0.9} strokeLinecap="round">
        <circle cx={0} cy={-3} r={0.9} />
        <line x1={0} y1={-2.2} x2={0} y2={3} />
        <line x1={-1.7} y1={-1.3} x2={1.7} y2={-1.3} />
        <path d="M -2.2 1.4 A 2.4 2.4 0 0 0 2.2 1.4" />
      </g>,
    );
  // Commando: a smaller ✕ (short of the box corners) with arrowheads atop the upper arms.
  if (has("Commando"))
    add(
      <g fill="none" stroke={S} strokeWidth={sw} strokeLinecap="round" strokeLinejoin="round">
        <line x1={-3} y1={-2.8} x2={3} y2={2.8} />
        <line x1={-3} y1={2.8} x2={3} y2={-2.8} />
        <polyline points="-2.53,-1.27 -3,-2.8 -1.44,-2.44" />
        <polyline points="2.53,-1.27 3,-2.8 1.44,-2.44" />
      </g>,
    );

  return parts.length ? <g>{frame}{parts}</g> : null;
}

export const PATTERNS = (
  <defs>
    <pattern id="patGravel" width={14} height={14} patternUnits="userSpaceOnUse">
      <rect width={14} height={14} fill={CREAM} />
      <g fill="none" stroke="#c0983c" strokeWidth={0.5}>
        <circle cx={3} cy={4} r={1.6} />
        <circle cx={9} cy={9} r={2} />
        <circle cx={11.5} cy={2.5} r={1.1} />
        <circle cx={5} cy={11.5} r={1.2} />
      </g>
      {/* scattered fine grit between the pebbles */}
      <g fill="#b8912f">
        <circle cx={6.5} cy={2} r={0.5} />
        <circle cx={1.5} cy={8} r={0.45} />
        <circle cx={12.5} cy={6.5} r={0.5} />
        <circle cx={7.5} cy={6} r={0.4} />
        <circle cx={10} cy={12.5} r={0.5} />
        <circle cx={2.5} cy={13} r={0.45} />
        <circle cx={13} cy={11} r={0.4} />
        <circle cx={5} cy={7.5} r={0.4} />
      </g>
    </pattern>
    {/* Salt pan: cream ground webbed by a network of golden cracks into irregular cells. Edge
        endpoints match across the tile (x0↔x36 at y=7/20/31; y0↔y36 at x=9/22/31) so it seams. */}
    <pattern id="patSaltMarsh" width={36} height={36} patternUnits="userSpaceOnUse">
      <rect width={36} height={36} fill="#e4dcac" />
      <g fill="none" stroke="#e6b81e" strokeWidth={2} strokeLinejoin="round" strokeLinecap="round">
        <path d="M0,7 L10,11 L22,8 L36,7" />
        <path d="M0,20 L9,18 L15,24 L26,21 L36,20" />
        <path d="M0,31 L8,29 L18,33 L30,30 L36,31" />
        <path d="M9,0 L10,11 L9,18 L8,29 L9,36" />
        <path d="M22,0 L22,8 L15,24 L18,33 L22,36" />
        <path d="M31,0 L26,21 L30,30 L31,36" />
      </g>
      <g fill="none" stroke="#f2cc22" strokeWidth={1} strokeLinejoin="round" strokeLinecap="round">
        <path d="M10,11 L15,24 M26,21 L18,33 M22,8 L26,21" />
      </g>
    </pattern>
    <pattern id="patVeg" width={12} height={12} patternUnits="userSpaceOnUse">
      <rect width={12} height={12} fill="#eef0c4" />
      <circle cx={3} cy={3} r={1.5} fill="#5f9e3b" />
      <circle cx={8} cy={6} r={1.7} fill="#7fae3a" />
      <circle cx={5} cy={9.5} r={1.3} fill="#b8c24a" />
      <circle cx={10.5} cy={10.5} r={1.2} fill="#5f9e3b" />
    </pattern>
    <pattern id="patSwamp" width={12} height={12} patternUnits="userSpaceOnUse">
      <rect width={12} height={12} fill="#e9edcf" />
      <path d="M2,3 v4 M6,6 v4 M9,2 v4 M4,9.5 v2.5 M10.5,8 v3" stroke="#4f8f63" strokeWidth={0.7} />
    </pattern>
    {/* Major City: a little skyscraper skyline sitting on a baseline. */}
    <pattern id="patCity" width={20} height={16} patternUnits="userSpaceOnUse">
      <rect width={20} height={16} fill={CREAM} />
      <g fill="#6f9ec6">
        <rect x={1} y={9} width={3} height={7} />
        <rect x={4.5} y={5} width={3} height={11} />
        <rect x={8} y={7.5} width={2.5} height={8.5} />
        <rect x={11} y={3} width={3} height={13} />
        <rect x={14.5} y={8} width={2.5} height={8} />
        <rect x={17.5} y={6} width={2.5} height={10} />
      </g>
      {/* windows */}
      <g fill={CREAM} opacity={0.8}>
        <rect x={5.3} y={7} width={0.8} height={0.8} />
        <rect x={6.4} y={7} width={0.8} height={0.8} />
        <rect x={5.3} y={9} width={0.8} height={0.8} />
        <rect x={11.8} y={5} width={0.8} height={0.8} />
        <rect x={12.9} y={5} width={0.8} height={0.8} />
        <rect x={11.8} y={7.5} width={0.8} height={0.8} />
        <rect x={12.9} y={7.5} width={0.8} height={0.8} />
      </g>
    </pattern>
    {/* Classic pictorial peaks (deviates from the paper, but reads as flowing ranges) —
        ~3 larger mountains per tile. */}
    <pattern id="patMountain" width={30} height={22} patternUnits="userSpaceOnUse">
      <rect width={30} height={22} fill="#8f7f42" />
      {/* three peaks fully contained in the tile (bases on the baseline so tiles seam cleanly) */}
      <g fill="#6f6330">
        <path d="M0,22 L6,8 L13,22 Z" />
        <path d="M9,22 L16,6 L23,22 Z" />
        <path d="M17,22 L24,9 L30,22 Z" />
      </g>
      {/* snow caps — base corners computed on each peak's (asymmetric) slopes */}
      <g fill="#e4dcb2">
        <path d="M6,8 L8,12 L4.3,12 Z" />
        <path d="M16,6 L18.2,11 L13.8,11 Z" />
        <path d="M24,9 L26.3,14 L21.3,14 Z" />
      </g>
    </pattern>
    {/* Desert: ochre with drifting sand-dune ridges. */}
    <pattern id="patDesert" width={24} height={14} patternUnits="userSpaceOnUse">
      <rect width={24} height={14} fill="#cc9a1f" />
      <g fill="none" strokeWidth={1}>
        <path d="M-2,9 Q6,3 12,9 T26,9" stroke="#b8871a" />
        <path d="M-2,9 Q6,4.5 12,9 T26,9" stroke="#e0b64e" opacity={0.7} />
        <path d="M-6,14 Q2,9 10,14 M12,14 Q20,9 28,14" stroke="#b8871a" />
      </g>
    </pattern>
    {/* Sea: light blue with little wave crests. */}
    <pattern id="patSea" width={16} height={12} patternUnits="userSpaceOnUse">
      <rect width={16} height={12} fill="#7ec2e8" />
      <g fill="none" strokeWidth={0.8} strokeLinecap="round">
        <path d="M1,4 q2.5,-2 5,0 t5,0" stroke="#aedcf5" />
        <path d="M3,9 q2.5,-2 5,0 t5,0" stroke="#5aa8d8" />
      </g>
    </pattern>
  </defs>
);
