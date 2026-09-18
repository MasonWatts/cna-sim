import { memo, useMemo } from "react";
import type { MapView } from "@bindings/MapView";
import type { UnitView } from "@bindings/UnitView";
import type { ReplacementView } from "@bindings/ReplacementView";
import type { SupplyDumpView } from "@bindings/SupplyDumpView";
import type { TruckConvoyView } from "@bindings/TruckConvoyView";
import type { RaiderView } from "@bindings/RaiderView";
import type { RommelView } from "@bindings/RommelView";
import type { MapOverlaysView } from "@bindings/MapOverlaysView";
import type { MinefieldView } from "@bindings/MinefieldView";
import type { AirView } from "@bindings/AirView";
import type { AirFacilityView } from "@bindings/AirFacilityView";
import type { SquadronView } from "@bindings/SquadronView";
import type { CapView } from "@bindings/CapView";
import type { NavalView } from "@bindings/NavalView";
import type { ShipView } from "@bindings/ShipView";
import { hexCenter, hexPolygon, mapBounds } from "../hex";

import { terrainFill, HEX_STROKE, ROUTE_DARK, ROUTE_KINDS, EDGE_KINDS, edgeGlyph, routeGlyph, featureGlyph, unitTypeGlyph, PATTERNS } from "./mapSymbols";


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
export const HexMap = memo(function HexMap({ map, units, weather, replacements, supplyDumps, truckConvoys, raiders, rommel, overlays, hexsides, air, naval, placeLore, onPlaceHover, onPlaceLeave, legend, unitKey }: { map: MapView; units: UnitView[]; weather?: string[]; replacements?: ReplacementView[]; supplyDumps?: SupplyDumpView[]; truckConvoys?: TruckConvoyView[]; raiders?: RaiderView[]; rommel?: RommelView[]; overlays?: MapOverlaysView; hexsides?: MapView["hexsides"]; air?: AirView; naval?: NavalView; placeLore?: Record<string, unknown>; onPlaceHover?: (name: string, x: number, y: number) => void; onPlaceLeave?: () => void; legend?: React.ReactNode; unitKey?: React.ReactNode }) {
  // Routes/edge-terrain fold forward during play (rail/road built or destroyed); fall back to the
  // static header map when no delta has been threaded.
  const hexsideList = hexsides ?? map.hexsides;
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
      (hexsideList ?? [])
        .filter((hs) => EDGE_KINDS.has(hs.kind))
        .map((hs, i) => {
          const A = hexCenter(hs.a[0], hs.a[1]);
          const B = hexCenter(hs.b[0], hs.b[1]);
          const highIsB = hs.high ? hs.high[0] === hs.b[0] && hs.high[1] === hs.b[1] : null;
          return <g key={i}>{edgeGlyph(hs.kind, A, B, highIsB)}</g>;
        }),
    [hexsideList],
  );

  // Linear routes (road/track/rail) — rebuilt when the hexside set changes (rail/road built/destroyed).
  const routes = useMemo(
    () =>
      (hexsideList ?? [])
        .filter((hs) => ROUTE_KINDS.has(hs.kind))
        .map((hs, i) => {
          const A = hexCenter(hs.a[0], hs.a[1]);
          const B = hexCenter(hs.b[0], hs.b[1]);
          return <g key={i}>{routeGlyph(hs.kind, A, B)}</g>;
        }),
    [hexsideList],
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
  const labels = useMemo(() => {
    // Collect label specs (position/text/style) without placing them yet.
    const specs = map.hexes.flatMap((hex) => {
      const feats = hex.features ?? [];
      const isBir = feats.includes("Bir");
      const text = isBir ? "Bir" : hex.label;
      if (!text) return [];
      const settlement = hex.terrain === "MajorCity" || feats.includes("Village") || isBir;
      const fontSize = settlement ? 11 : 10.5;
      // Courier New is monospace (~0.62em advance); letter-spacing adds a little on POIs.
      const width = text.length * fontSize * 0.62 + (settlement ? 0 : text.length * 0.5);
      return [{ hex, text, settlement, fontSize, width, c: hexCenter(hex.col, hex.row) }];
    });
    // Place higher-priority labels first (settlements over area/POI annotations); ties broken
    // deterministically by position so the layout is stable across renders.
    specs.sort(
      (a, b) => Number(b.settlement) - Number(a.settlement) || a.c.y - b.c.y || a.c.x - b.c.x,
    );

    // Candidate anchor placements relative to the hex centre, tried in order of preference.
    type Box = { x0: number; y0: number; x1: number; y1: number };
    const candidates: { dx: number; dy: number; anchor: "middle" | "start" | "end" }[] = [
      { dx: 0, dy: -18, anchor: "middle" }, // default: above
      { dx: 0, dy: 22, anchor: "middle" }, // below
      { dx: 0, dy: -30, anchor: "middle" }, // higher
      { dx: 0, dy: 34, anchor: "middle" }, // lower
      { dx: 15, dy: -4, anchor: "start" }, // right
      { dx: -15, dy: -4, anchor: "end" }, // left
    ];
    const PAD = 1.5;
    const placed: Box[] = [];
    const overlaps = (a: Box, b: Box) =>
      a.x0 < b.x1 + PAD && a.x1 + PAD > b.x0 && a.y0 < b.y1 + PAD && a.y1 + PAD > b.y0;
    const boxFor = (s: (typeof specs)[number], cand: (typeof candidates)[number]): Box => {
      const ax = s.c.x + cand.dx;
      const ay = s.c.y + cand.dy;
      const left = cand.anchor === "middle" ? ax - s.width / 2 : cand.anchor === "start" ? ax : ax - s.width;
      return { x0: left, y0: ay - s.fontSize * 0.9, x1: left + s.width, y1: ay + s.fontSize * 0.3 };
    };

    // POIs (regions/oases/tracks/Nile) render centred on their hex, not offset. Reserve their
    // centre boxes up front so settlement labels still route around them.
    for (const s of specs) {
      if (!s.settlement) {
        placed.push({ x0: s.c.x - s.width / 2, y0: s.c.y - s.fontSize * 0.6, x1: s.c.x + s.width / 2, y1: s.c.y + s.fontSize * 0.4 });
      }
    }

    return specs.map((s) => {
      const key = `lbl-${s.hex.col},${s.hex.row}`;
      // A place with a lore entry (keyed by its label text) becomes an interactive "read more" label.
      const hasLore = !!placeLore && Object.prototype.hasOwnProperty.call(placeLore, s.text);
      const hover = hasLore
        ? {
            // Anchor the panel to the label's on-screen box (fixed spot), not the moving cursor, so it
            // stays put and is easy to mouse into for the links.
            onMouseEnter: (e: React.MouseEvent) => {
              const r = (e.currentTarget as SVGGraphicsElement).getBoundingClientRect();
              onPlaceHover?.(s.text, r.left, r.bottom);
            },
            onMouseLeave: () => onPlaceLeave?.(),
          }
        : {};
      if (!s.settlement) {
        return (
          <text
            key={key}
            x={s.c.x}
            y={s.c.y}
            textAnchor="middle"
            dominantBaseline="central"
            fontFamily="Courier New, monospace"
            fontSize={10.5}
            fontWeight={700}
            fontStyle="italic"
            fill="#7a6a45"
            stroke="#efe8c6"
            strokeWidth={2.2}
            paintOrder="stroke"
            style={{ pointerEvents: hasLore ? "auto" : "none", cursor: hasLore ? "help" : undefined, letterSpacing: "0.5px" }}
            {...hover}
          >
            {s.text}
          </text>
        );
      }
      // Settlements: pick the first candidate that clears every placed label; fall back to default.
      let chosen = candidates[0];
      for (const cand of candidates) {
        if (!placed.some((p) => overlaps(p, boxFor(s, cand)))) {
          chosen = cand;
          break;
        }
      }
      placed.push(boxFor(s, chosen));
      const x = s.c.x + chosen.dx;
      const y = s.c.y + chosen.dy;
      // A small caret from the label toward its hex — apex points at the hex; base sits just off
      // the label on the hex-facing side, so it works for whichever placement was chosen.
      const cx = chosen.anchor === "middle" ? x : chosen.anchor === "start" ? x + s.width / 2 : x - s.width / 2;
      const cy = y - s.fontSize * 0.35; // visual centre of the text
      const dx = s.c.x - cx;
      const dy = s.c.y - cy;
      const len = Math.hypot(dx, dy) || 1;
      const ux = dx / len;
      const uy = dy / len;
      const gap = s.fontSize * 0.55 + 0.5; // clear the text (sit close to it)
      const bx = cx + ux * gap;
      const by = cy + uy * gap;
      const ax = bx + ux * 5; // apex, 5px toward the hex
      const ay = by + uy * 5;
      const px = -uy * 3; // perpendicular half-base
      const py = ux * 3;
      const caret = `${ax.toFixed(1)},${ay.toFixed(1)} ${(bx - px).toFixed(1)},${(by - py).toFixed(1)} ${(bx + px).toFixed(1)},${(by + py).toFixed(1)}`;
      return (
        <g key={key} style={{ pointerEvents: "none" }}>
          <polygon points={caret} fill={ROUTE_DARK} stroke="#efe8c6" strokeWidth={1} paintOrder="stroke" />
          <text
            x={x}
            y={y}
            textAnchor={chosen.anchor}
            fontFamily="Courier New, monospace"
            fontSize={11}
            fontWeight={700}
            fill={ROUTE_DARK}
            stroke="#efe8c6"
            strokeWidth={1.8}
            paintOrder="stroke"
            style={hasLore ? { pointerEvents: "auto", cursor: "help" } : undefined}
            {...hover}
          >
            {s.text}
          </text>
        </g>
      );
    });
  }, [map, placeLore, onPlaceHover, onPlaceLeave]);

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

  // Group every hex occupant — combat units, free truck convoys (§53), and on-map Replacement-Point
  // pools (§20) — into one per-hex stack so they render as counters like the paper game (a "top"
  // counter + a +N badge) rather than crowding the hex with separate glyphs. A convoy hauling an
  // Infantry pool stacks with it automatically (they are co-located), showing the pool as a
  // subordinate counter; a self-moving or not-yet-picked-up pool stands alone as its own counter.
  type Tile =
    | { kind: "unit"; rank: number; u: UnitView }
    | { kind: "rommel"; rank: number; m: RommelView }
    | { kind: "raider"; rank: number; k: RaiderView }
    | { kind: "convoy"; rank: number; c: TruckConvoyView }
    | { kind: "rp"; rank: number; r: ReplacementView }
    | { kind: "dump"; rank: number; d: SupplyDumpView }
    | { kind: "mine"; rank: number; f: MinefieldView }
    | { kind: "pipe"; rank: number; p: [number, number] }
    | { kind: "squadron"; rank: number; sq: SquadronView }
    | { kind: "cap"; rank: number; cp: CapView }
    | { kind: "airfield"; rank: number; af: AirFacilityView }
    | { kind: "ship"; rank: number; sh: ShipView };
  const byHex = new Map<string, Tile[]>();
  const pushTile = (hex: [number, number] | null | undefined, t: Tile) => {
    if (!hex) return;
    const key = `${hex[0]},${hex[1]}`;
    (byHex.get(key) ?? byHex.set(key, []).get(key)!).push(t);
  };
  // Ranks: Rommel > unit > raider > convoy > pool > dump > minefield > pipeline.
  for (const m of rommel ?? []) pushTile(m.hex, { kind: "rommel", rank: 200, m });
  for (const u of units) pushTile(u.hex as [number, number] | undefined, { kind: "unit", rank: 100 + orgRank(u.org_size), u });
  for (const k of raiders ?? []) pushTile(k.hex, { kind: "raider", rank: 60, k });
  for (const c of truckConvoys ?? []) pushTile(c.hex, { kind: "convoy", rank: 50, c });
  for (const r of replacements ?? []) if (r.hex) pushTile(r.hex, { kind: "rp", rank: 10, r });
  for (const d of supplyDumps ?? []) pushTile(d.hex, { kind: "dump", rank: 5, d });
  for (const f of overlays?.minefields ?? []) pushTile(f.hex, { kind: "mine", rank: 2, f });
  for (const p of overlays?.pipelines ?? []) pushTile(p as [number, number], { kind: "pipe", rank: 1, p: p as [number, number] });
  // Air layer: squadrons and CAP patrols rank above ground raiders/convoys; airfields are infrastructure.
  for (const sq of air?.squadrons ?? []) pushTile(sq.hex, { kind: "squadron", rank: 70, sq });
  for (const cp of air?.caps ?? []) pushTile(cp.hex, { kind: "cap", rank: 65, cp });
  for (const af of air?.facilities ?? []) pushTile(af.hex, { kind: "airfield", rank: 4, af });
  // Warships on the map (§30): a bombarding fleet unit or the San Giorgio wreck.
  for (const sh of naval?.ships ?? []) pushTile(sh.hex, { kind: "ship", rank: 80, sh });

  return (
    <svg width={w} height={h + LABEL_HEADROOM} viewBox={`${minX} ${minY - LABEL_HEADROOM} ${w} ${h + LABEL_HEADROOM}`} style={{ display: "block" }} role="img" aria-label="game board">
      {PATTERNS}
      {terrain}
      {sections}
      {edges}
      {routes}
      {featureGlyphs}
      {labels}
      {/* Map overlays (§24.4 fortifications): hex-background glyphs drawn under the counters.
          Minefields and pipelines render as their own lowest-rank stack tiles. */}
      {overlays && (
        <g>
          {overlays.fortifications.map((f) => {
            const c = hexCenter(f.hex[0], f.hex[1]);
            return (
              <g key={`ft${f.hex[0]},${f.hex[1]}`} transform={`translate(${c.x} ${c.y - 14})`}>
                {/* a bunker arc with the fortification level */}
                <path d="M-8,4 L-8,-2 A8,6 0 0 1 8,-2 L8,4" fill="none" stroke="#6b5a33" strokeWidth={1.8} />
                <text x={0} y={3} textAnchor="middle" fontSize={7} fontWeight={700} fill="#6b5a33">{f.level}</text>
                {f.damage > 0 && <circle cx={9} cy={-2} r={2} fill="#c0392b" />}
                <title>{`Fortification level ${f.level} (§24.4)${f.damage > 0 ? `, damage ${f.damage}` : ""} @ (${f.hex[0]},${f.hex[1]})`}</title>
              </g>
            );
          })}
        </g>
      )}
      {/* Occupant counters: one representative "top" counter per hex with a +N badge for the rest of
          the stack. Combat units, free convoys, and Replacement pools all render as tiles here. */}
      {[...byHex.entries()].map(([key, stack]) => {
        const [col, row] = key.split(",").map(Number);
        const ctr = hexCenter(col, row);
        const sorted = [...stack].sort((a, b) => b.rank - a.rank);
        const top = sorted[0];
        const extra = stack.length - 1;
        const sideOf = (t: Tile) =>
          t.kind === "unit" ? t.u.side
          : t.kind === "rommel" ? "Axis"
          : t.kind === "raider" ? t.k.side
          : t.kind === "convoy" ? t.c.side
          : t.kind === "rp" ? t.r.side
          : t.kind === "dump" ? t.d.side
          : t.kind === "mine" ? t.f.side
          : t.kind === "squadron" ? t.sq.side
          : t.kind === "cap" ? t.cp.side
          : t.kind === "ship" ? t.sh.side
          : ""; // pipeline / airfield: no side

        const side = sideOf(top);
        // Pipelines / airfields aren't side-owned — tint pipe water-blue, airfield slate; else by side.
        const fill = top.kind === "pipe" ? "#4a6b78" : top.kind === "airfield" ? "#556070" : top.kind === "ship" && top.sh.wreck ? "#6b6b6b" : side === "Commonwealth" ? "var(--cw)" : "var(--axis)";
        // Status modifiers (broken 🔧 / training 🎓 / dashed border) reflect only the TOP counter's own
        // state — a subordinate tile's status lives in the tooltip, not painted over the representative.
        const topBroken = (top.kind === "unit" && top.u.broken) || (top.kind === "convoy" && top.c.broken > 0);
        const topTraining = (top.kind === "unit" && top.u.in_training) || (top.kind === "rp" && top.r.training_remaining > 0);
        // 0-TOE HQ cadre (§19.62/§19.67) or a still-training pool render with a dashed (provisional)
        // border. A §32.18 dummy dump is deliberately indistinguishable from a real one (solid border,
        // same glyph) — only its value line reads "decoy" for the omniscient viewer.
        const isCadre = top.kind === "unit" && top.u.toe === 0;
        const isTrainingRp = top.kind === "rp" && top.r.training_remaining > 0;
        // §24.9: loose supplies (not a constructed dump) render provisional too. A dummy dump is built,
        // so it stays solid.
        const isLooseDump = top.kind === "dump" && !top.d.constructed;
        // §27.32: a concealed raider (not revealed to the enemy) draws provisional too.
        const isHiddenRaider = top.kind === "raider" && !top.k.revealed;
        // §26.15: a minefield whose type the enemy hasn't tested (not revealed) draws dashed; once
        // revealed (an enemy stepped on it), it's solid.
        const isProvisionalMine = top.kind === "mine" && !top.f.revealed;
        // §35.14: an unsupplied squadron draws provisional (can't refit its planes).
        const isUnsuppliedSqn = top.kind === "squadron" && !top.sq.supplied;
        // A scuttled wreck (San Giorgio) draws provisional.
        const isWreck = top.kind === "ship" && top.sh.wreck;
        const dashed = isCadre || isTrainingRp || isLooseDump || isHiddenRaider || isProvisionalMine || isUnsuppliedSqn || isWreck;
        const isRommel = top.kind === "rommel";
        const clsShort = (c: string) => (c === "Infantry" ? "Inf" : c);
        const moraleStr = (u: UnitView) =>
          `${u.basic_morale >= 0 ? "+" : ""}${u.basic_morale}` +
          (u.basic_morale < u.morale_reference ? `→${u.morale_reference >= 0 ? "+" : ""}${u.morale_reference}` : "") +
          (u.in_training ? " 🎓" : "");
        // One tooltip line per occupant, kind-appropriate.
        const line = (t: Tile): string => {
          if (t.kind === "unit")
            return `• ${t.u.name} (${t.u.org_size}, TOE ${t.u.toe}, morale ${moraleStr(t.u)})${t.u.broken ? " 🔧" : ""}`;
          if (t.kind === "rommel")
            return `• Rommel (§31)${t.m.attached_to != null ? ` — lending CPA to unit #${t.m.attached_to}` : ""}`;
          if (t.kind === "raider")
            return `• ${t.k.kind === "Lrdg" ? "LRDG" : "Sonderkommando"} desert raider (§27) — ${t.k.side}, fuel ${t.k.fuel}, ammo ${t.k.ammo}${t.k.revealed ? "" : " — concealed"}`;
          if (t.kind === "convoy") {
            const cargo = [t.c.fuel ? `F${t.c.fuel}` : "", t.c.ammo ? `A${t.c.ammo}` : "", t.c.stores ? `S${t.c.stores}` : "", t.c.water ? `W${t.c.water}` : ""].filter(Boolean).join(" ") || "empty";
            return `• Truck Convoy — ${t.c.light + t.c.medium + t.c.heavy} TP (L${t.c.light} M${t.c.medium} H${t.c.heavy})${t.c.broken ? `, ${t.c.broken} broken 🔧` : ""}; cargo ${cargo}${t.c.carries_replacements ? "; hauling RP" : ""}`;
          }
          if (t.kind === "rp") {
            const st = t.r.training_remaining > 0 ? `training ${t.r.training_remaining} stage(s) 🎓` : "trained";
            return `• ${clsShort(t.r.class)} RP ×${t.r.count} (${st})${t.r.carried_by != null ? " — loaded on convoy" : " — awaiting pickup"}`;
          }
          if (t.kind === "dump") {
            const dc = [t.d.fuel ? `Fuel ${t.d.fuel}` : "", t.d.ammo ? `Ammo ${t.d.ammo}` : "", t.d.stores ? `Stores ${t.d.stores}` : "", t.d.water ? `Water ${t.d.water}` : ""].filter(Boolean).join(", ") || "empty";
            const kind = t.d.dummy ? "Dummy dump (§32.18)" : t.d.constructed ? "Supply Dump (§24.9)" : "Loose supplies (§24.9 — not convoy-loadable)";
            return `• ${kind} — ${t.d.dummy ? "no real supplies" : dc}`;
          }
          if (t.kind === "mine") {
            return `• ${t.f.dummy ? "Dummy minefield" : "Minefield"} (§26) — ${t.f.side}${t.f.revealed ? "" : ", unspotted by the enemy"}`;
          }
          if (t.kind === "pipe") return `• Water pipeline (§52.2) — unlimited water while connected to a source`;
          if (t.kind === "squadron")
            return `• ${t.sq.designation} squadron (§35) — ${t.sq.side} ${t.sq.class}, ${t.sq.planes} plane(s)${t.sq.supplied ? "" : " — UNSUPPLIED (§35.14)"}`;
          if (t.kind === "cap") return `• Combat Air Patrol (§40) — ${t.cp.side} ${t.cp.cap_type}`;
          if (t.kind === "airfield") return `• ${t.af.kind} (§36) — capacity ${t.af.capacity} squadron(s)`;
          // ship
          return t.sh.wreck
            ? `• ${t.sh.name} — scuttled wreck (§55.25), gun ${t.sh.gun}/${t.sh.max_gun}`
            : `• ${t.sh.name} — ${t.sh.side} warship at sea (§30), gun ${t.sh.gun}/${t.sh.max_gun}`;
        };
        const tip = `${side} @ (${col},${row})${stack.length > 1 ? ` — ${stack.length} counters` : ""}\n` + sorted.map(line).join("\n");
        // The representative counter's two text rows: a title line and a value/glyph line.
        const body = (() => {
          if (top.kind === "unit") {
            return (
              <>
                <text textAnchor="middle" y={-1} fontSize={9} fill="#fff" fontWeight={700}>{top.u.name.slice(0, 7)}</text>
                <text x={isCadre ? 0 : -9.5} textAnchor="middle" y={8} fontSize={isCadre ? 7 : 8} fill="#eef" fontStyle={isCadre ? "italic" : "normal"}>
                  {isCadre ? "cadre" : top.u.toe}
                </text>
                {!isCadre && top.u.unit_type.length > 0 && <g transform="translate(9.5 5)">{unitTypeGlyph(top.u.unit_type)}</g>}
              </>
            );
          }
          if (top.kind === "rommel") {
            return (
              <>
                <text textAnchor="middle" y={-2} fontSize={8.5} fill="#fff" fontWeight={700}>Rommel</text>
                {/* a gold command star, centered inside the counter box */}
                <path d="M0,-4 L1.18,-1.24 L4.05,-1.24 L1.76,0.62 L2.6,3.4 L0,1.7 L-2.6,3.4 L-1.76,0.62 L-4.05,-1.24 L-1.18,-1.24 Z" transform="translate(0 5)" fill="#e8c04a" stroke="#7a5c12" strokeWidth={0.4} />
              </>
            );
          }
          if (top.kind === "raider") {
            return (
              <>
                <text textAnchor="middle" y={-1} fontSize={8.5} fill="#fff" fontWeight={700}>{top.k.kind === "Lrdg" ? "LRDG" : "DSA"}</text>
                <text textAnchor="middle" y={8} fontSize={7} fill="#eef">{`F${top.k.fuel} A${top.k.ammo}`}</text>
              </>
            );
          }
          if (top.kind === "convoy") {
            return (
              <>
                <text textAnchor="middle" y={-1} fontSize={8.5} fill="#fff" fontWeight={700}>Convoy</text>
                <text x={-9.5} textAnchor="middle" y={8} fontSize={8} fill="#eef">{`${top.c.light + top.c.medium + top.c.heavy}TP`}</text>
                {/* a small lorry facing right at the glyph slot */}
                <g transform="translate(9.5 5)" stroke="#0d0f14" strokeWidth={0.7}>
                  <rect x={-6} y={-3.5} width={7} height={5} rx={0.5} fill="#e9e4cf" />
                  <rect x={1} y={-1.5} width={3.5} height={3} rx={0.5} fill="#e9e4cf" />
                  <circle cx={-3.5} cy={2.5} r={1} fill="#0d0f14" stroke="none" />
                  <circle cx={3} cy={2.5} r={1} fill="#0d0f14" stroke="none" />
                </g>
              </>
            );
          }
          if (top.kind === "rp") {
            return (
              <>
                <text textAnchor="middle" y={-1} fontSize={8.5} fill="#fff" fontWeight={700}>{`${clsShort(top.r.class)} RP`}</text>
                <text textAnchor="middle" y={8} fontSize={8} fill="#eef">{`×${top.r.count}`}</text>
                {isTrainingRp && <text x={15} y={-3} fontSize={7} fill="#c8b06a" fontWeight={700}>{top.r.training_remaining}</text>}
              </>
            );
          }
          if (top.kind === "mine") {
            return (
              <>
                <text textAnchor="middle" y={-1} fontSize={8.5} fill="#fff" fontWeight={700}>Mines</text>
                <text textAnchor="middle" y={8} fontSize={8} fill="#eef">{top.f.dummy ? "decoy" : "active"}</text>
              </>
            );
          }
          if (top.kind === "pipe") {
            return (
              <>
                <text textAnchor="middle" y={-1} fontSize={8.5} fill="#fff" fontWeight={700}>Pipe</text>
                <text x={-9.5} textAnchor="middle" y={8} fontSize={7} fill="#eef">water</text>
                {/* a short pipe segment with end flanges at the glyph slot */}
                <g transform="translate(9.5 5)" stroke="#cfe6ef" strokeWidth={1.4} strokeLinecap="round">
                  <line x1={-5} y1={0} x2={5} y2={0} />
                  <line x1={-5} y1={-2} x2={-5} y2={2} strokeWidth={1} />
                  <line x1={5} y1={-2} x2={5} y2={2} strokeWidth={1} />
                </g>
              </>
            );
          }
          if (top.kind === "squadron") {
            return (
              <>
                <text textAnchor="middle" y={-1} fontSize={8} fill="#fff" fontWeight={700}>{top.sq.designation.slice(0, 8)}</text>
                <text x={-9.5} textAnchor="middle" y={8} fontSize={7} fill="#eef">{`✈${top.sq.planes}`}</text>
                {/* a small plane silhouette */}
                <g transform="translate(9.5 5)" fill="#e9e4cf" stroke="#0d0f14" strokeWidth={0.4}>
                  <path d="M0,-3.5 L1,-0.5 L5,1 L1,1.5 L0.7,4 L-0.7,4 L-1,1.5 L-5,1 L-1,-0.5 Z" />
                </g>
              </>
            );
          }
          if (top.kind === "cap") {
            return (
              <>
                <text textAnchor="middle" y={-1} fontSize={8.5} fill="#fff" fontWeight={700}>CAP</text>
                <text textAnchor="middle" y={8} fontSize={7} fill="#eef">{top.cp.cap_type.slice(0, 9)}</text>
              </>
            );
          }
          if (top.kind === "airfield") {
            const short = top.af.kind === "Airfield" ? "Airfield" : top.af.kind.includes("Strip") ? "Airstrip" : top.af.kind.includes("Boat") ? "Seaplane" : "Airfield";
            return (
              <>
                <text textAnchor="middle" y={-1} fontSize={8} fill="#fff" fontWeight={700}>{short}</text>
                <text x={-9.5} textAnchor="middle" y={8} fontSize={7} fill="#eef">{`cap ${top.af.capacity}`}</text>
                {/* a runway with center dashes */}
                <g transform="translate(9.5 5)" stroke="#e9e4cf" strokeWidth={1.2} strokeLinecap="round">
                  <line x1={-5} y1={2.5} x2={5} y2={-2.5} />
                  <line x1={-2} y1={0.6} x2={-0.5} y2={-0.3} strokeWidth={0.7} />
                  <line x1={1} y1={-0.2} x2={2.5} y2={-1.1} strokeWidth={0.7} />
                </g>
              </>
            );
          }
          if (top.kind === "ship") {
            return (
              <>
                <text textAnchor="middle" y={-1} fontSize={8} fill="#fff" fontWeight={700}>{top.sh.wreck ? "Wreck" : top.sh.name.slice(0, 8)}</text>
                <text x={-9.5} textAnchor="middle" y={8} fontSize={7} fill="#eef">{`⚓${top.sh.gun}`}</text>
                {/* a ship hull silhouette */}
                <g transform="translate(9.5 5)" fill="#e9e4cf" stroke="#0d0f14" strokeWidth={0.4}>
                  <path d="M-6,0 L6,0 L4,3 L-4,3 Z" />
                  <rect x={-1.5} y={-3} width={3} height={3} />
                </g>
              </>
            );
          }
          // dump: "Depot" + a 50-gal drum beside a crate. A §32.18 dummy looks identical to a real dump;
          // its value line reads "decoy" where a real one shows contents.
          return (
            <>
              <text textAnchor="middle" y={-1} fontSize={8.5} fill="#fff" fontWeight={700}>{top.d.constructed ? "Depot" : "Cache"}</text>
              <text x={-6} textAnchor="middle" y={8} fontSize={7} fill="#eef">
                {top.d.dummy ? "decoy" : `×${top.d.fuel + top.d.ammo + top.d.stores + top.d.water}`}
              </text>
              {/* a 50-gal drum (rim ellipse + two hoop bands) beside a crate (diagonal slats) */}
              <g transform="translate(12 5)" stroke="#0d0f14" strokeWidth={0.6}>
                <rect x={-6} y={-3} width={4} height={7} rx={0.9} fill="#e9e4cf" />
                <ellipse cx={-4} cy={-3} rx={2} ry={0.7} fill="#e9e4cf" />
                <line x1={-6} y1={-0.6} x2={-2} y2={-0.6} strokeWidth={0.4} />
                <line x1={-6} y1={1.6} x2={-2} y2={1.6} strokeWidth={0.4} />
                <rect x={-1.5} y={-1.5} width={5.5} height={5.5} rx={0.5} fill="#e9e4cf" />
                <line x1={-1.5} y1={-1.5} x2={4} y2={4} strokeWidth={0.4} />
                <line x1={4} y1={-1.5} x2={-1.5} y2={4} strokeWidth={0.4} />
              </g>
            </>
          );
        })();
        return (
          <g key={key} transform={`translate(${ctr.x} ${ctr.y})`}>
            {/* a hint of the stack underneath the top counter */}
            {extra > 0 && (
              <rect x={-17} y={-13} width={38} height={22} rx={3} fill={fill} opacity={0.5} stroke="#0d0f14" strokeWidth={1} />
            )}
            <rect
              x={-19}
              y={-11}
              width={38}
              height={22}
              rx={3}
              fill={fill}
              fillOpacity={isCadre ? 0.45 : 1}
              stroke={isRommel ? "#e8c04a" : topBroken ? "#e7b53a" : "#0d0f14"}
              strokeWidth={isRommel || topBroken ? 2 : 1}
              strokeDasharray={dashed ? "3 2" : undefined}
            />
            {body}
            {topBroken && <text x={-15} y={-4} fontSize={11} textAnchor="middle" aria-label="broken down">🔧</text>}
            {topTraining && <text x={-15} y={7} fontSize={9} textAnchor="middle" aria-label="in training">🎓</text>}
            {/* +N badge for the rest of the stack */}
            {extra > 0 && (
              <g transform="translate(19 -11)">
                <circle r={8} fill="#11151c" stroke="#eef" strokeWidth={1} />
                <text textAnchor="middle" y={3} fontSize={9} fill="#eef" fontWeight={700}>+{extra}</text>
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
      {/* Replacement-Point delivery: a dashed line from each assigned on-map pool toward its unit's
          hex (§20.41/§20.49). The pools themselves render as counters in the hex stack above; staged
          future arrivals (§20.21) are listed in a panel. */}
      {replacements && replacements.length > 0 && (() => {
        const unitHex = new Map<number, [number, number]>();
        for (const u of units) if (u.hex) unitHex.set(u.id, u.hex as [number, number]);
        const onMap = replacements.filter((r) => r.hex);
        const staged = replacements.filter((r) => !r.hex);
        return (
          <g>
            {onMap.map((r, i) => {
              if (r.assigned_to == null) return null;
              const dst = unitHex.get(r.assigned_to);
              if (!dst || (dst[0] === r.hex![0] && dst[1] === r.hex![1])) return null;
              const a = hexCenter(r.hex![0], r.hex![1]);
              const b = hexCenter(dst[0], dst[1]);
              return <line key={`rpl${i}`} x1={a.x} y1={a.y} x2={b.x} y2={b.y} stroke="#c8b06a" strokeWidth={1.2} strokeDasharray="4 3" opacity={0.6} />;
            })}
            {staged.length > 0 && (
              <g>
                <rect x={panelX - 234} y={panelY} width={220} height={28 + staged.length * 15} rx={6} fill="#efe8c6" stroke="#b7a97a" strokeWidth={1} opacity={0.96} />
                <text x={panelX - 124} y={panelY + 17} textAnchor="middle" fontFamily="Courier New, monospace" fontSize={13} fontWeight={700} fill="#2f2b22">Incoming Replacements</text>
                {staged.map((r, i) => (
                  <text key={i} x={panelX - 224} y={panelY + 34 + i * 15} fontFamily="Courier New, monospace" fontSize={10} fill="#5b5340">
                    {`GT ${r.arrival ? `${r.arrival[0]}/${r.arrival[1]}` : "?"} — ${r.side.slice(0, 2)} ${r.class} ×${r.count}`}
                  </text>
                ))}
              </g>
            )}
          </g>
        );
      })()}
      {/* Naval panel (§55/§56): port efficiencies and scheduled Axis convoys — abstract/named naval
          state that has no map hex, so it lives in an off-map panel (like Incoming Replacements). */}
      {naval && (naval.ports.length > 0 || naval.convoys.length > 0) && (() => {
        const rows = naval.ports.length + naval.convoys.length + (naval.convoys.length ? 1 : 0);
        const px = panelX - 234;
        const py = panelY + 150;
        let y = py + 34;
        return (
          <g>
            <rect x={px} y={py} width={220} height={28 + rows * 14} rx={6} fill="#e6eef0" stroke="#7a97a0" strokeWidth={1} opacity={0.96} />
            <text x={px + 110} y={py + 17} textAnchor="middle" fontFamily="Courier New, monospace" fontSize={13} fontWeight={700} fill="#243033">Naval (§55/§56)</text>
            {naval.ports.map((p, i) => {
              const yy = y + i * 14;
              return (
                <text key={`np${i}`} x={px + 10} y={yy} fontFamily="Courier New, monospace" fontSize={10} fill={p.blocked ? "#a33" : "#3a4b4f"}>
                  {`${p.name}: eff ${p.efficiency}/${p.max_efficiency}${p.blocked ? " ⚠blocked" : ""}`}
                </text>
              );
            })}
            {naval.convoys.length > 0 && (() => {
              y = py + 34 + naval.ports.length * 14;
              return (
                <>
                  <text x={px + 10} y={y + 2} fontFamily="Courier New, monospace" fontSize={9} fontWeight={700} fill="#243033">Axis convoys:</text>
                  {naval.convoys.map((c, i) => (
                    <text key={`nc${i}`} x={px + 10} y={y + 16 + i * 14} fontFamily="Courier New, monospace" fontSize={10} fill="#3a4b4f">
                      {`GT${c.arrival_gt} L${c.lane}→${c.destination} ${c.tons}t`}
                    </text>
                  ))}
                </>
              );
            })()}
          </g>
        );
      })()}
      {/* Legend inset drawn in map coordinates (foreignObject) so it pans/zooms with the board. */}
      {legend && (
        <foreignObject x={legendX} y={legendY} width={LW} height={LH} style={{ overflow: "visible" }}>
          <div style={{ display: "flex", flexDirection: "column", gap: 12, alignItems: "flex-start" }}>
            {legend}
          </div>
        </foreignObject>
      )}
      {/* Unit Key: below the weather panel, left of the Terrain Key. */}
      {unitKey && (
        <foreignObject x={panelX} y={panelY + WPH + 12} width={WPW} height={200} style={{ overflow: "visible" }}>
          {unitKey}
        </foreignObject>
      )}
    </svg>
  );
});
