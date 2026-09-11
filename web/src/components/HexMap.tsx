import { memo, useMemo } from "react";
import type { MapView } from "@bindings/MapView";
import type { UnitView } from "@bindings/UnitView";
import { hexCenter, hexPolygon, mapBounds } from "../hex";

// Paper-map palette (parchment base). Solid fills for flat terrains; the rest reference an SVG
// <pattern> (see PATTERNS) so textured terrains match the printed key.
const CREAM = "#efe8c6";
const TERRAIN_FILL: Record<string, string> = {
  Clear: CREAM,
  Sea: "#7ec2e8",
  Desert: "#cc9a1f",
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
    </pattern>
    <pattern id="patSaltMarsh" width={18} height={18} patternUnits="userSpaceOnUse">
      <rect width={18} height={18} fill={CREAM} />
      <ellipse cx={7} cy={9} rx={5} ry={3.5} fill="#cdd69a" opacity={0.55} />
      <g fill="none" stroke="#d6c23f" strokeWidth={1}>
        <path d="M0,5 Q5,2 9,6 T18,5" />
        <path d="M0,13 Q5,16 10,11 T18,13" />
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
    <pattern id="patCity" width={14} height={14} patternUnits="userSpaceOnUse">
      <rect width={14} height={14} fill={CREAM} />
      <g fill="#6f9ec6">
        <rect x={2} y={2} width={3} height={2} />
        <rect x={7} y={3} width={2} height={3} />
        <rect x={10} y={6} width={3} height={2} />
        <rect x={3} y={8.5} width={2} height={3} />
        <rect x={8} y={10} width={3} height={2} />
      </g>
    </pattern>
    <pattern id="patMountain" width={18} height={18} patternUnits="userSpaceOnUse">
      <rect width={18} height={18} fill="#8a7c3e" />
      <ellipse cx={6} cy={5} rx={5} ry={3} fill="#6f6330" opacity={0.6} />
      <path d="M0,12 Q5,8 9,12 T18,11 L18,18 L0,18 Z" fill="#6f6330" opacity={0.55} />
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
