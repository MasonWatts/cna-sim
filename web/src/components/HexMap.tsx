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

// The 5 map sections span contiguous global-column bands (A=1–33, B=34–66, C=67–99, D=100–132,
// E=133–165), the same `(col-1)/33` split the weather engine uses.
const SECTION_BOUNDS = [33, 66, 99, 132]; // last col of A, B, C, D
const SECTION_CENTERS = [17, 50, 83, 116, 149]; // representative mid-column of A…E
const LABEL_HEADROOM = 52; // extra viewBox space above the map so Region labels clear all coastlines

/** A small pictorial weather icon (~radius 6) centered at (0,0), for the weather panel. */
function weatherGlyph(kind: string): JSX.Element {
  const rays = [0, 45, 90, 135, 180, 225, 270, 315].map((d, i) => {
    const a = (d * Math.PI) / 180;
    return (
      <line key={i} x1={Math.cos(a) * 6} y1={Math.sin(a) * 6} x2={Math.cos(a) * 8.5} y2={Math.sin(a) * 8.5} />
    );
  });
  switch (kind) {
    case "Hot":
      return (
        <g>
          <g stroke="#d9782d" strokeWidth={1.5} strokeLinecap="round">{rays}</g>
          <circle r={5} fill="#eb9d34" stroke="#d9782d" strokeWidth={1} />
        </g>
      );
    case "Sandstorm":
      return (
        <g stroke="#b8863a" strokeWidth={1.7} fill="none" strokeLinecap="round">
          <path d="M-8,-4 q6,-3.5 13,0" />
          <path d="M-9,0 q7,-3.5 15,0" />
          <path d="M-8,4 q6,-3.5 13,0" />
        </g>
      );
    case "Rainstorm":
      return (
        <g>
          <path d="M-7,2 a4,4 0 0 1 3.5,-5.2 a5,5 0 0 1 9.3,1 a3.4,3.4 0 0 1 -1.2,6.8 h-10.4 a3,3 0 0 1 -1.2,-3.6 z" fill="#8b97a4" stroke="#65707c" strokeWidth={0.8} />
          <g stroke="#4f86c6" strokeWidth={1.4} strokeLinecap="round">
            <line x1={-4} y1={5} x2={-5.5} y2={8.5} />
            <line x1={0} y1={5} x2={-1.5} y2={8.5} />
            <line x1={4} y1={5} x2={2.5} y2={8.5} />
          </g>
        </g>
      );
    default: // Normal / clear
      return (
        <g>
          <g stroke="#c9a227" strokeWidth={1.3} strokeLinecap="round">{rays}</g>
          <circle r={4.5} fill="#e8c750" stroke="#c9a227" strokeWidth={0.8} />
        </g>
      );
  }
}

/** The board at one step: static terrain hexes + the units currently on them. Rendered at natural
 *  content size; the parent applies a CSS transform for pan/zoom (SVG scales crisply).
 *
 *  Perf: `memo`'d so pan/zoom (which only change the parent's transform, not these props) skip it
 *  entirely, and the ~7k-polygon terrain layer is `useMemo`'d on `map` so scrubbing (units change)
 *  never rebuilds it — only the small units layer re-renders. */
export const HexMap = memo(function HexMap({ map, units, weather, legend }: { map: MapView; units: UnitView[]; weather?: string[]; legend?: React.ReactNode }) {
  const { minX, minY, w, h } = useMemo(() => mapBounds(map.hexes), [map]);
  // Off-shore info boxes (in map coordinates, so they pan/zoom with the board): the Weather panel
  // and, just to its right, the Terrain Key — both shifted right of centre so they clear the
  // Section C region label at the top-centre.
  const WPW = 250; // weather panel width
  const WPH = 104; // weather panel height (glyph + condition name)
  const panelX = minX + w / 2 - 40;
  const panelY = minY + 58;
  const LW = 400;
  const LH = 360;
  const legendX = panelX + WPW + 24;
  const legendY = panelY;

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

  // Cosmetic map labels. Two styles:
  //  • Settlement (Major City / Village / Bir): upright serif place-name above the hex.
  //  • POI (a labelled hex that is none of those — regions, oases, tracks, the Nile): italic,
  //    muted brown, centered on the hex, letter-spaced so it reads as an area/feature annotation.
  // Built once per map.
  const labels = useMemo(
    () =>
      map.hexes.flatMap((hex) => {
        const feats = hex.features ?? [];
        const isBir = feats.includes("Bir");
        const text = isBir ? "Bir" : hex.label;
        if (!text) return [];
        const c = hexCenter(hex.col, hex.row);
        const settlement = hex.terrain === "MajorCity" || feats.includes("Village") || isBir;
        return [
          settlement ? (
            <text
              key={`lbl-${hex.col},${hex.row}`}
              x={c.x}
              y={c.y - 18}
              textAnchor="middle"
              fontFamily="Courier New, monospace"
              fontSize={11}
              fontWeight={700}
              fill={ROUTE_DARK}
              stroke="#efe8c6"
              strokeWidth={1.8}
              paintOrder="stroke"
              style={{ pointerEvents: "none" }}
            >
              {text}
            </text>
          ) : (
            <text
              key={`lbl-${hex.col},${hex.row}`}
              x={c.x}
              y={c.y - 18}
              textAnchor="middle"
              fontFamily="Courier New, monospace"
              fontSize={10.5}
              fontWeight={700}
              fontStyle="italic"
              fill="#7a6a45"
              stroke="#efe8c6"
              strokeWidth={2.2}
              paintOrder="stroke"
              style={{ pointerEvents: "none", letterSpacing: "0.5px" }}
            >
              {text}
            </text>
          ),
        ];
      }),
    [map],
  );

  // Faint section boundaries + A–E letters, so the weather panel's per-section cells can be tied
  // back to map regions (the stitched map otherwise shows no section nomenclature). Static per map.
  const sections = useMemo(() => {
    const lines = SECTION_BOUNDS.map((b, i) => {
      const x = (hexCenter(b, 0).x + hexCenter(b + 1, 0).x) / 2;
      return <line key={i} x1={x} y1={minY} x2={x} y2={minY + h} stroke="#8f8663" strokeWidth={1.2} strokeDasharray="6 6" opacity={0.5} />;
    });
    const letters = SECTION_CENTERS.map((c, i) => (
      <text key={i} x={hexCenter(c, 0).x} y={minY - LABEL_HEADROOM + 40} textAnchor="middle" fontFamily="Courier New, monospace" fontSize={44} fontWeight={700} fill="#8f8663" opacity={0.4} style={{ pointerEvents: "none" }}>
        {`Region ${String.fromCharCode(65 + i)}`}
      </text>
    ));
    return [...lines, ...letters];
  }, [minX, minY, w, h]);

  // Group units by hex so we can fan out stacks.
  const byHex = new Map<string, UnitView[]>();
  for (const u of units) {
    if (!u.hex) continue;
    const key = `${u.hex[0]},${u.hex[1]}`;
    (byHex.get(key) ?? byHex.set(key, []).get(key)!).push(u);
  }

  return (
    <svg width={w} height={h + LABEL_HEADROOM} viewBox={`${minX} ${minY - LABEL_HEADROOM} ${w} ${h + LABEL_HEADROOM}`} style={{ display: "block" }} role="img" aria-label="game board">
      {PATTERNS}
      {terrain}
      {sections}
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
      {/* Weather panel: one cell per section (A–E), pictorial condition. Floats in the off-map
          zone (top-left), pans/zooms with the board; the on-map A–E dividers tie cells to regions. */}
      {(() => {
        const px = panelX;
        const py = panelY;
        const cellW = (WPW - 16) / 5;
        return (
          <g>
            <rect x={px} y={py} width={WPW} height={WPH} rx={6} fill="#efe8c6" stroke="#b7a97a" strokeWidth={1} opacity={0.96} />
            <text x={px + WPW / 2} y={py + 17} textAnchor="middle" fontFamily="Courier New, monospace" fontSize={13} fontWeight={700} fill="#2f2b22">
              Weather
            </text>
            {SECTION_CENTERS.map((_, i) => {
              const cx = px + 8 + cellW * (i + 0.5);
              const cond = weather?.[i] ?? "Normal";
              return (
                <g key={i}>
                  <text x={cx} y={py + 36} textAnchor="middle" fontFamily="Courier New, monospace" fontSize={12} fontWeight={700} fill="#5b5340">
                    {String.fromCharCode(65 + i)}
                  </text>
                  <g transform={`translate(${cx} ${py + 58})`}>{weatherGlyph(cond)}</g>
                  <text x={cx} y={py + 92} textAnchor="middle" fontFamily="Courier New, monospace" fontSize={8.5} fill="#5b5340">
                    {cond}
                  </text>
                  <title>{`Section ${String.fromCharCode(65 + i)}: ${cond}`}</title>
                </g>
              );
            })}
          </g>
        );
      })()}
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
