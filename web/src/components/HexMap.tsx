import { memo, useMemo } from "react";
import type { MapView } from "@bindings/MapView";
import type { UnitView } from "@bindings/UnitView";
import { hexCenter, hexPolygon, mapBounds } from "../hex";

import { terrainFill, HEX_STROKE, ROUTE_DARK, ROUTE_KINDS, EDGE_KINDS, edgeGlyph, routeGlyph, featureGlyph, PATTERNS } from "./mapSymbols";


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
export const HexMap = memo(function HexMap({ map, units, legend }: { map: MapView; units: UnitView[]; legend?: React.ReactNode }) {
  const { minX, minY, w, h } = useMemo(() => mapBounds(map.hexes), [map]);
  // Legend inset in map coordinates: centered horizontally, up in the sea (high rows are north →
  // top of the content) so it floats a bit off-shore. Sized in content units; scales with the map.
  const LW = 400;
  const LH = 360;
  const legendX = minX + w / 2 - LW / 2;
  const legendY = minY + 24;

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

  // Edge-terrain features (rivers/wadi/ridge/escarpment/slope) drawn on the shared edge — built
  // once per map, under the routes so a road/rail crossing draws on top.
  const edges = useMemo(
    () =>
      (map.hexsides ?? [])
        .filter((hs) => EDGE_KINDS.has(hs.kind))
        .map((hs, i) => {
          const A = hexCenter(hs.a[0], hs.a[1]);
          const B = hexCenter(hs.b[0], hs.b[1]);
          const highIsB = hs.high ? hs.high[0] === hs.b[0] && hs.high[1] === hs.b[1] : null;
          return <g key={i}>{edgeGlyph(hs.kind, A, B, highIsB)}</g>;
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

  // Cosmetic place-name labels (paper-map style): a serif name above the hex. Birs show a generic
  // "Bir"; Villages/Major Cities show their transcribed `label` when present. Built once per map.
  const labels = useMemo(
    () =>
      map.hexes.flatMap((hex) => {
        const isBir = (hex.features ?? []).includes("Bir");
        const text = isBir ? "Bir" : hex.label;
        if (!text) return [];
        const c = hexCenter(hex.col, hex.row);
        return [
          <text
            key={`lbl-${hex.col},${hex.row}`}
            x={c.x}
            y={c.y - 18}
            textAnchor="middle"
            fontFamily="Georgia, 'Times New Roman', serif"
            fontSize={11}
            fill={ROUTE_DARK}
            stroke="#efe8c6"
            strokeWidth={1.4}
            paintOrder="stroke"
            style={{ pointerEvents: "none" }}
          >
            {text}
          </text>,
        ];
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
      {edges}
      {routes}
      {featureGlyphs}
      {labels}
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
        const anyTraining = stack.some((u) => u.in_training);
        const isCadre = top.toe === 0; // a 0-TOE HQ shown as its formation's cadre (§19.62/§19.67)
        const fill = top.side === "Commonwealth" ? "var(--cw)" : "var(--axis)";
        // Morale: show "current" and, for a green unit training up (§17.31), "current→trained".
        const moraleStr = (u: UnitView) =>
          `${u.basic_morale >= 0 ? "+" : ""}${u.basic_morale}` +
          (u.basic_morale < u.morale_reference ? `→${u.morale_reference >= 0 ? "+" : ""}${u.morale_reference}` : "") +
          (u.in_training ? " 🎓" : "");
        const tip =
          stack.length === 1
            ? `${top.name} — ${top.side}${top.broken ? " ⚠ BROKEN DOWN (§21)" : ""}${top.in_training ? " 🎓 TRAINING (§17.3)" : ""}\nTOE ${top.toe}  fuel ${top.fuel}  ammo ${top.ammo}  cohesion ${top.cohesion}  morale ${moraleStr(top)}\n${top.org_size} @ (${col},${row})`
            : `${stack.length} units @ (${col},${row}) — ${top.side}\n` +
              [...stack]
                .sort((a, b) => orgRank(b.org_size) - orgRank(a.org_size) || b.toe - a.toe)
                .map((u) => `• ${u.name} (${u.org_size}, TOE ${u.toe}, morale ${moraleStr(u)})${u.broken ? " 🔧" : ""}`)
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
            {anyTraining && (
              <text x={-11} y={7} fontSize={9} textAnchor="middle" aria-label="in training">
                🎓
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
      {/* Legend inset drawn in map coordinates (foreignObject) so it pans/zooms with the board. */}
      {legend && (
        <foreignObject x={legendX} y={legendY} width={LW} height={LH} style={{ overflow: "visible" }}>
          <div className="legend-map">
            {legend}
          </div>
        </foreignObject>
      )}
    </svg>
  );
});
