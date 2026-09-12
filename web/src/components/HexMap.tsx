import { memo, useMemo } from "react";
import type { MapView } from "@bindings/MapView";
import type { UnitView } from "@bindings/UnitView";
import { hexCenter, hexPolygon, mapBounds } from "../hex";

// Paper-map palette (parchment base). Solid fills for flat terrains; the rest reference an SVG
// <pattern> (see PATTERNS) so textured terrains match the printed key.
const CREAM = "#efe8c6";
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
const terrainFill = (t: string) => TERRAIN_FILL[t] ?? CREAM;
const HEX_STROKE = "#b7ad82"; // faint parchment-gray hexside, like the paper map
const FEATURE_BLUE = "#3f8fc4"; // the paper legend's feature blue (Oasis palms, Port anchor, dots)

const ROUTE_DARK = "#4a4644";
const RAIL_BLUE = "#4a9ad0";
const ROUTE_KINDS = new Set(["Road", "UnfinishedRoad", "Track", "Railroad", "UnfinishedRailroad"]);

/** A linear route (road/track/rail) drawn center-to-center across the shared edge of two adjacent
 *  hexes A and B. Roads are double lines, tracks a single dashed line, rails a line with cross-ties;
 *  the "Unfinished" variants dash the line. */
function routeGlyph(kind: string, A: { x: number; y: number }, B: { x: number; y: number }): JSX.Element {
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
  // Railroad / UnfinishedRailroad: a line with perpendicular cross-ties.
  const dashed = kind === "UnfinishedRailroad";
  const n = Math.max(2, Math.round(len / 6));
  const ties = [];
  for (let i = 1; i < n; i++) {
    const t = i / n;
    const px = A.x + dx * t;
    const py = A.y + dy * t;
    ties.push(<line key={i} x1={px + nx * 2.6} y1={py + ny * 2.6} x2={px - nx * 2.6} y2={py - ny * 2.6} />);
  }
  return (
    <g stroke={RAIL_BLUE} strokeWidth={1.1} strokeLinecap="round">
      <line x1={A.x} y1={A.y} x2={B.x} y2={B.y} strokeDasharray={dashed ? "4 3" : undefined} />
      {ties}
    </g>
  );
}

/** Point-feature glyph centered at (0,0) — the caller translates it to the hex center.
 *  Oasis (palms on a pool) and Port (anchor in a ring) mimic the paper legend; Bir and Village are
 *  a small filled dot for now (text labels to differentiate/identify come later). */
function featureGlyph(feature: string): JSX.Element | null {
  switch (feature) {
    case "Bir":
    case "Village":
      return <circle r={3.5} fill={FEATURE_BLUE} />;
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
const PATTERNS = (
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

// Rank org sizes so the largest formation in a stack is shown as the "top" counter.
const ORG_RANK: Record<string, number> = {
  Division: 6,
  SuperBrigade: 5,
  StandardBrigade: 4,
  BattleGroup: 3,
  Battalion: 2,
  Company: 1,
};
const orgRank = (o: string) => ORG_RANK[o] ?? 0;

/** The board at one step: static terrain hexes + the units currently on them. Rendered at natural
 *  content size; the parent applies a CSS transform for pan/zoom (SVG scales crisply).
 *
 *  Perf: `memo`'d so pan/zoom (which only change the parent's transform, not these props) skip it
 *  entirely, and the ~7k-polygon terrain layer is `useMemo`'d on `map` so scrubbing (units change)
 *  never rebuilds it — only the small units layer re-renders. */
export const HexMap = memo(function HexMap({ map, units }: { map: MapView; units: UnitView[] }) {
  const { minX, minY, w, h } = useMemo(() => mapBounds(map.hexes), [map]);

  // Static terrain layer — built once per map, reused across every step.
  const terrain = useMemo(
    () =>
      map.hexes.map((hex) => {
        const c = hexCenter(hex.col, hex.row);
        return (
          <polygon
            key={`${hex.col},${hex.row}`}
            points={hexPolygon(c.x, c.y)}
            fill={terrainFill(hex.terrain)}
            stroke={HEX_STROKE}
            strokeWidth={1}
          >
            <title>{`${hex.terrain} (${hex.col},${hex.row})`}</title>
          </polygon>
        );
      }),
    [map],
  );

  // Linear routes (road/track/rail) — built once per map.
  const routes = useMemo(
    () =>
      (map.hexsides ?? [])
        .filter((hs) => ROUTE_KINDS.has(hs.kind))
        .map((hs, i) => {
          const A = hexCenter(hs.a[0], hs.a[1]);
          const B = hexCenter(hs.b[0], hs.b[1]);
          return <g key={i}>{routeGlyph(hs.kind, A, B)}</g>;
        }),
    [map],
  );

  // Static point-feature glyphs (Oasis/Village/Bir/Port) — built once per map like the terrain.
  const featureGlyphs = useMemo(
    () =>
      map.hexes.flatMap((hex) =>
        (hex.features ?? []).map((f) => {
          const g = featureGlyph(f);
          if (!g) return null;
          const c = hexCenter(hex.col, hex.row);
          return (
            <g key={`${hex.col},${hex.row},${f}`} transform={`translate(${c.x} ${c.y})`}>
              {g}
            </g>
          );
        }),
      ),
    [map],
  );

  // Group units by hex so we can fan out stacks.
  const byHex = new Map<string, UnitView[]>();
  for (const u of units) {
    if (!u.hex) continue;
    const key = `${u.hex[0]},${u.hex[1]}`;
    (byHex.get(key) ?? byHex.set(key, []).get(key)!).push(u);
  }

  return (
    <svg width={w} height={h} viewBox={`${minX} ${minY} ${w} ${h}`} style={{ display: "block" }} role="img" aria-label="game board">
      {PATTERNS}
      {terrain}
      {routes}
      {featureGlyphs}
      {/* units: one "top" counter per hex with a +N badge for the rest of the stack */}
      {[...byHex.entries()].map(([key, stack]) => {
        const [col, row] = key.split(",").map(Number);
        const c = hexCenter(col, row);
        // Pick the representative: largest org size, then highest TOE, then id (stable).
        const top = [...stack].sort(
          (a, b) => orgRank(b.org_size) - orgRank(a.org_size) || b.toe - a.toe || a.id - b.id,
        )[0];
        const extra = stack.length - 1;
        const anyBroken = stack.some((u) => u.broken);
        const isCadre = top.toe === 0; // a 0-TOE HQ shown as its formation's cadre (§19.62/§19.67)
        const fill = top.side === "Commonwealth" ? "var(--cw)" : "var(--axis)";
        const tip =
          stack.length === 1
            ? `${top.name} — ${top.side}${top.broken ? " ⚠ BROKEN DOWN (§21)" : ""}\nTOE ${top.toe}  fuel ${top.fuel}  ammo ${top.ammo}  cohesion ${top.cohesion}\n${top.org_size} @ (${col},${row})`
            : `${stack.length} units @ (${col},${row}) — ${top.side}\n` +
              [...stack]
                .sort((a, b) => orgRank(b.org_size) - orgRank(a.org_size) || b.toe - a.toe)
                .map((u) => `• ${u.name} (${u.org_size}, TOE ${u.toe})${u.broken ? " 🔧" : ""}`)
                .join("\n");
        return (
          <g key={key} transform={`translate(${c.x} ${c.y})`}>
            {/* a hint of the stack underneath the top counter */}
            {extra > 0 && (
              <rect x={-13} y={-13} width={30} height={22} rx={3} fill={fill} opacity={0.5} stroke="#0d0f14" strokeWidth={1} />
            )}
            <rect
              x={-15}
              y={-11}
              width={30}
              height={22}
              rx={3}
              fill={fill}
              fillOpacity={isCadre ? 0.45 : 1}
              stroke={anyBroken ? "#e7b53a" : "#0d0f14"}
              strokeWidth={anyBroken ? 2 : 1}
              strokeDasharray={isCadre ? "3 2" : undefined}
            />
            <text textAnchor="middle" y={-1} fontSize={9} fill="#fff" fontWeight={700}>
              {top.name.slice(0, 7)}
            </text>
            <text textAnchor="middle" y={8} fontSize={isCadre ? 7 : 8} fill="#eef" fontStyle={isCadre ? "italic" : "normal"}>
              {isCadre ? "cadre" : top.toe}
            </text>
            {anyBroken && (
              <text x={-11} y={-4} fontSize={11} textAnchor="middle" aria-label="broken down">
                🔧
              </text>
            )}
            {/* +N badge for the rest of the stack */}
            {extra > 0 && (
              <g transform="translate(15 -11)">
                <circle r={8} fill="#11151c" stroke="#eef" strokeWidth={1} />
                <text textAnchor="middle" y={3} fontSize={9} fill="#eef" fontWeight={700}>
                  +{extra}
                </text>
              </g>
            )}
            <title>{tip}</title>
          </g>
        );
      })}
    </svg>
  );
});
