import { memo, useMemo } from "react";
import type { MapView } from "@bindings/MapView";
import type { UnitView } from "@bindings/UnitView";
import { hexCenter, hexPolygon, mapBounds } from "../hex";

const TERRAIN_FILL: Record<string, string> = {
  Clear: "#2a2f1c",
  Desert: "#3a3320",
  Sand: "#3a3320",
  Rough: "#3a2f26",
  Mountain: "#3b3540",
  Escarpment: "#332a2a",
  Salt: "#26323a",
  Sea: "#1a2b3a",
  Oasis: "#1f3a2a",
};
const terrainFill = (t: string) => TERRAIN_FILL[t] ?? "#242a1e";

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
            stroke="#0d0f14"
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
