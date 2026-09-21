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
import type { PoisonedWellView } from "@bindings/PoisonedWellView";
import type { AirView } from "@bindings/AirView";
import type { AirFacilityView } from "@bindings/AirFacilityView";
import type { SquadronView } from "@bindings/SquadronView";
import type { CapView } from "@bindings/CapView";
import type { NavalView } from "@bindings/NavalView";
import type { ShipView } from "@bindings/ShipView";
import type { EntryDetail, InfoPanelData, StackEntry, StackStateRow } from "../lore";
import { WEATHER_SYSTEM_INFO, WEATHER_CONDITION_INFO } from "../weatherInfo";
import { PORT_SYSTEM_INFO, PORT_TONNAGE_INFO, PORT_TRANSFER_INFO, CONVOY_SYSTEM_INFO, OFF_MAP_INFO, OFF_MAP_AA_INFO, OFF_MAP_PORT_INFO, OFF_MAP_TRANSIT_INFO, MED_BASE_INFO, MALTA_INFO, MALTA_CAPACITY_INFO, MALTA_AA_INFO, CW_OFF_MAP_INFO, facilityInfo } from "../navalInfo";
import type { OffMapView } from "@bindings/OffMapView";
import type { OffMapUnitView } from "@bindings/OffMapUnitView";
import type { PrisonerView } from "@bindings/PrisonerView";
import type { CapturedView } from "@bindings/CapturedView";
import type { DummyTankView } from "@bindings/DummyTankView";
import type { BrokenVehicleView } from "@bindings/BrokenVehicleView";
import { hexCenter, hexPolygon, mapBounds, parseHexId } from "../hex";

import { terrainFill, HEX_STROKE, ROUTE_DARK, ROUTE_KINDS, EDGE_KINDS, edgeGlyph, routeGlyph, featureGlyph, unitTypeGlyph, squadronGlyph, shipGlyph, airfieldGlyph, PATTERNS } from "./mapSymbols";


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

// A counter title that shrinks its font size to fit the counter's width (±19 → ~34px usable). Short
// titles keep the base size; a long one (e.g. a historical German squadron name like "8/III JG 27")
// scales down uniformly instead of being clipped. Uses an estimated glyph width (~0.6 × size for the
// bold sans face) so no DOM measurement is needed; floored at 5px so it never vanishes.
function TileTitle({ text, y = -1, size = 8.5, max = 34, fill = "#fff" }: { text: string; y?: number; size?: number; max?: number; fill?: string }) {
  const factor = 0.6;
  const est = text.length * size * factor;
  const fs = est > max ? Math.max(5, max / (text.length * factor)) : size;
  return (
    <text textAnchor="middle" y={y} fontSize={fs} fill={fill} fontWeight={700}>
      {text}
    </text>
  );
}

// A squadron counter (§35) as drawn on the board — side-colored 38×22 tile, designation title,
// plane count, class silhouette, dashed if unsupplied, with a +N badge for the rest of a stack.
// Centered at the origin; the parent positions it. Reused for on-map hexes and Malta/off-map boxes.
function SquadronCounter({ designation, planes, cls, side = "Commonwealth", supplied = true, extra = 0, title, noTitle }: { designation: string; planes: number; cls: string; side?: string; supplied?: boolean; extra?: number; title?: string; noTitle?: boolean }) {
  const fill = side === "Commonwealth" ? "var(--cw)" : "var(--axis)";
  return (
    <g>
      {extra > 0 && <rect x={-17} y={-13} width={38} height={22} rx={3} fill={fill} opacity={0.5} stroke="#0d0f14" strokeWidth={1} />}
      <rect x={-19} y={-11} width={38} height={22} rx={3} fill={fill} stroke="#0d0f14" strokeWidth={1} strokeDasharray={supplied ? undefined : "3 2"} />
      <TileTitle text={designation.replace(/^(Commonwealth|Axis|Italian|German)\s+/, "")} y={-1} size={8.5} />
      <text x={-9.5} textAnchor="middle" y={8} fontSize={7} fill="#eef">{`x${planes}`}</text>
      <g transform="translate(9.5 4.5) scale(0.62)" fill="#e9e4cf" stroke="#0d0f14" strokeWidth={0.5}>{squadronGlyph(cls)}</g>
      {extra > 0 && (
        <g transform="translate(19 -11)">
          <circle r={8} fill="#11151c" stroke="#eef" strokeWidth={1} />
          <text textAnchor="middle" y={3} fontSize={9} fill="#eef" fontWeight={700}>+{extra}</text>
        </g>
      )}
      {!noTitle && <title>{title ?? `• ${designation} squadron (§35) — ${side} ${cls}, ${planes} plane(s)${supplied ? "" : " — UNSUPPLIED (§35.14)"}`}</title>}
    </g>
  );
}

// A full land-unit counter (§4.44) as drawn on the board — side-colored 38×22 tile, abbr title, TOE
// (— for a 0-TOE cadre/HQ), the composed type glyph, broken/training markers, and a native tooltip.
// Centered at the origin; the parent positions it. Reused on the map and in the off-map complex.
function UnitCounter({ u, extra = 0, noTitle }: { u: UnitView; extra?: number; noTitle?: boolean }) {
  const fill = u.side === "Commonwealth" ? "var(--cw)" : "var(--axis)";
  const cadre = u.toe === 0;
  const morale = `${u.basic_morale >= 0 ? "+" : ""}${u.basic_morale}` + (u.basic_morale < u.morale_reference ? `→${u.morale_reference >= 0 ? "+" : ""}${u.morale_reference}` : "");
  const tip = `• ${u.name} (${u.org_size}, TOE ${cadre ? "-" : u.toe}, morale ${morale})${u.broken ? " 🔧" : ""}${u.in_training ? " 🎓" : ""}`;
  return (
    <g>
      {extra > 0 && <rect x={-17} y={-13} width={38} height={22} rx={3} fill={fill} opacity={0.5} stroke="#0d0f14" strokeWidth={1} />}
      <rect x={-19} y={-11} width={38} height={22} rx={3} fill={fill} stroke={u.broken ? "#e7b53a" : "#0d0f14"} strokeWidth={u.broken ? 2 : 1} />
      <TileTitle text={u.abbr || u.name} y={-1} size={9} />
      <text x={-9.5} textAnchor="middle" y={8} fontSize={8} fill="#eef">{cadre ? "-" : u.toe}</text>
      {u.unit_type.length > 0 && <g transform="translate(9.5 5)">{unitTypeGlyph(u.unit_type)}</g>}
      {u.broken && <text x={-15} y={-4} fontSize={11} textAnchor="middle" aria-label="broken down">🔧</text>}
      {u.in_training && <text x={-15} y={7} fontSize={9} textAnchor="middle" aria-label="in training">🎓</text>}
      {extra > 0 && (
        <g transform="translate(19 -11)">
          <circle r={8} fill="#11151c" stroke="#eef" strokeWidth={1} />
          <text textAnchor="middle" y={3} fontSize={9} fill="#eef" fontWeight={700}>+{extra}</text>
        </g>
      )}
      {!noTitle && <title>{tip}</title>}
    </g>
  );
}

// The 5 map sections span contiguous global-column bands (A=1–33, B=34–66, C=67–99, D=100–132,
// E=133–165), the same `(col-1)/33` split the weather engine uses.
const SECTION_BOUNDS = [33, 66, 99, 132]; // last col of A, B, C, D
const SECTION_CENTERS = [17, 50, 83, 116, 149]; // representative mid-column of A…E
const LABEL_HEADROOM = 52; // extra viewBox space above the map so Region labels clear all coastlines
const LEFT_MARGIN = 1140; // extra viewBox space west of the map for the §8.8 Tripoli/Tunisia complex
const RIGHT_MARGIN = 220; // extra viewBox space east of the map for the §36.53 Canal-zone air boxes
const BOTTOM_MARGIN = 120; // extra viewBox space south of the map for the Ethiopia off-map box

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
export const HexMap = memo(function HexMap({ map, units, weather, replacements, supplyDumps, truckConvoys, raiders, rommel, overlays, hexsides, air, naval, offMap, prisoners, captured, dummyTanks, brokenVehicles, placeLore, onPlaceHover, onPlaceLeave, onInfoHover, legend, unitKey }: { map: MapView; units: UnitView[]; weather?: string[]; replacements?: ReplacementView[]; supplyDumps?: SupplyDumpView[]; truckConvoys?: TruckConvoyView[]; raiders?: RaiderView[]; rommel?: RommelView[]; overlays?: MapOverlaysView; hexsides?: MapView["hexsides"]; air?: AirView; naval?: NavalView; offMap?: OffMapView; prisoners?: PrisonerView[]; captured?: CapturedView[]; dummyTanks?: DummyTankView[]; brokenVehicles?: BrokenVehicleView[]; placeLore?: Record<string, unknown>; onPlaceHover?: (name: string, x: number, y: number) => void; onPlaceLeave?: () => void; onInfoHover?: (data: InfoPanelData, x: number, y: number) => void; legend?: React.ReactNode; unitKey?: React.ReactNode }) {
  // Generic hover-attrs for an SVG element that should open the hybrid info panel, anchored to the
  // element's on-screen box; reuses onPlaceLeave (the shared hide) for leaving.
  const infoHover = (data: InfoPanelData) =>
    onInfoHover
      ? {
          style: { cursor: "help" as const },
          onMouseEnter: (e: React.MouseEvent) => {
            const r = (e.currentTarget as SVGGraphicsElement).getBoundingClientRect();
            onInfoHover(data, r.left, r.bottom);
          },
          onMouseLeave: () => onPlaceLeave?.(),
        }
      : {};
  // Historical-context lore (public/lore/places.json) for a place label, as an info-panel section.
  const loreOfPlace = (key: string) => {
    const e = placeLore?.[key] as { description?: string; links?: { label: string; url: string }[] } | undefined;
    return e?.description ? { description: e.description, links: e.links } : undefined;
  };
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
    | { kind: "well"; rank: number; w: PoisonedWellView }
    | { kind: "pipe"; rank: number; p: [number, number] }
    | { kind: "squadron"; rank: number; sq: SquadronView }
    | { kind: "cap"; rank: number; cp: CapView }
    | { kind: "airfield"; rank: number; af: AirFacilityView }
    | { kind: "ship"; rank: number; sh: ShipView }
    | { kind: "prisoner"; rank: number; pr: PrisonerView }
    | { kind: "captured"; rank: number; ce: CapturedView }
    | { kind: "dummytank"; rank: number; dt: DummyTankView }
    | { kind: "broken"; rank: number; bv: BrokenVehicleView };
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
  for (const w of overlays?.poisoned_wells ?? []) pushTile(w.hex, { kind: "well", rank: 3, w });
  for (const p of overlays?.pipelines ?? []) pushTile(p as [number, number], { kind: "pipe", rank: 1, p: p as [number, number] });
  // Air layer: squadrons and CAP patrols rank above ground raiders/convoys; airfields are infrastructure.
  for (const sq of air?.squadrons ?? []) pushTile(sq.hex, { kind: "squadron", rank: 70, sq });
  for (const cp of air?.caps ?? []) pushTile(cp.hex, { kind: "cap", rank: 65, cp });
  for (const af of air?.facilities ?? []) pushTile(af.hex, { kind: "airfield", rank: 4, af });
  // Warships on the map (§30): a bombarding fleet unit or the San Giorgio wreck.
  for (const sh of naval?.ships ?? []) pushTile(sh.hex, { kind: "ship", rank: 80, sh });
  // §28 prisoner stacks + escapees — low rank (a guarding/nearby combat unit shows as the top tile).
  for (const pr of prisoners ?? []) pushTile(pr.hex, { kind: "prisoner", rank: 12, pr });
  // §28.3/§14.46 captured equipment + knocked-out tank markers — low rank infrastructure.
  for (const ce of captured ?? []) pushTile(ce.hex, { kind: "captured", rank: 8, ce });
  // §32 dummy (decoy) tanks and §21.5 broken-down vehicle markers — low rank.
  for (const dt of dummyTanks ?? []) pushTile(dt.hex, { kind: "dummytank", rank: 14, dt });
  for (const bv of brokenVehicles ?? []) pushTile(bv.hex, { kind: "broken", rank: 9, bv });

  // ── Shared detail-panel builders ────────────────────────────────────────────────────────────
  // Unit and squadron detail are reused both by the on-map stack loop and by the off-map complex
  // (land-transit boxes + off-map airfields), so they live at component scope. Kept as the single
  // source of truth: the loop's detailOf delegates here for these two kinds.
  const clsShort = (c: string) => (c === "Infantry" ? "Inf" : c);
  const moraleStr = (u: UnitView) =>
    `${u.basic_morale >= 0 ? "+" : ""}${u.basic_morale}` +
    (u.basic_morale < u.morale_reference ? `→${u.morale_reference >= 0 ? "+" : ""}${u.morale_reference}` : "") +
    (u.in_training ? " 🎓" : "");
  const unitLabel = (u: UnitView) => u.abbr || u.name;
  const unitDetail = (u: UnitView): EntryDetail => {
    const typeLabel = u.unit_type.length
      ? u.unit_type.map((k) => (k === "Hq" ? "HQ" : k === "AntiTank" ? "Anti-tank" : k === "AntiAircraft" ? "AA" : k)).join(" ")
      : "Unclassified";
    const rules = {
      description: u.is_hq
        ? "A Headquarters counter (§19) commands its formation and lends its Command Point Allowance to subordinate units. Reduced to 0 TOE it is not eliminated — it persists as the formation cadre (§19.62/§19.67), able to receive newly built battalions to rebuild."
        : "A land unit (§9) holds its hex, projects a Zone of Control into the six adjacent hexes (§10), and stacks up to the hex limit (§9.14). Combat strength scales with its TOE Strength Points (§4.4); TOE losses accumulate until the unit is eliminated at 0 (§19.62). It burns fuel to move and ammo to fight (§21) and must trace supply to a source (§23); poor morale or cohesion risks failed activation and combat (§17).",
    };
    const combat = [
      u.attack || u.defense ? `Assault ${u.attack}/${u.defense}` : "",
      u.anti_armor ? `Anti-armor ${u.anti_armor}` : "",
      u.barrage ? `Barrage ${u.barrage}` : "",
    ].filter(Boolean).join(" · ");
    const movement =
      u.cpa == null
        ? "—"
        : `${Math.max(u.cpa - u.cp_expended, 0).toFixed(1)} / ${u.cpa} CP${u.motorized ? " (motorized)" : " (leg)"}`;
    const state: StackStateRow[] = [
      { label: "Unit", value: u.name },
      { label: "Nationality", value: u.nationality },
      { label: "Size", value: u.org_size },
      { label: "Type", value: typeLabel },
      {
        label: "Formation",
        value: u.formation
          ? `${u.formation}${u.formation_colocated == null ? "" : u.formation_colocated ? " · co-located" : " · detached"}`
          : u.is_hq ? "— (formation HQ)" : "Independent",
      },
    ];
    if (u.attached_to) state.push({ label: "Attached to", value: u.attached_to });
    const carried = u.attached_units ?? [];
    if (carried.length) state.push({ label: "Carries", value: carried.join(", ") });
    state.push(
      { label: "TOE", value: u.toe === 0 ? (u.is_hq ? "0 (cadre)" : "0") : String(u.toe) },
      { label: "Combat", value: combat || "none" },
      { label: "Movement", value: movement },
      { label: "Supply", value: u.supplied ? "Adequate (§23)" : "Short (§23)" },
      { label: "Fuel", value: String(u.fuel) },
      { label: "Ammo", value: String(u.ammo) },
      { label: "Morale", value: moraleStr(u) },
      { label: "Cohesion", value: String(u.cohesion) },
    );
    const flags = [
      u.broken ? "Broken down (§21) — awaiting repair" : "",
      u.in_training ? "In training (§17.3)" : "",
    ].filter(Boolean);
    if (flags.length) state.push({ label: "Status", value: flags.join("; ") });
    const sideFill = u.side === "Commonwealth" ? "var(--cw)" : "var(--axis)";
    const glyph = u.unit_type.length ? (
      <svg width={46} height={30} viewBox="-14 -9 28 18" style={{ display: "block" }} aria-hidden="true">
        <rect x={-13} y={-8.5} width={26} height={17} rx={2.5} fill={sideFill} stroke="#0d0f14" strokeWidth={1} />
        {unitTypeGlyph(u.unit_type)}
      </svg>
    ) : undefined;
    return { glyph, rules, state };
  };
  type SqLike = { designation: string; class: string; planes: number; supplied: boolean };
  const squadronLabel = (sq: SqLike) => `${sq.designation.replace(/^(Commonwealth|Axis|Italian|German)\s+/, "")} sqn`;
  const squadronDetail = (sq: SqLike): EntryDetail => ({
    rules: {
      description:
        "An air squadron / SGSU (§35) flies missions from an air facility within range. It must be supplied to refit lost planes (§35.14) — an unsupplied squadron cannot replace losses; planes are drawn from and returned to the air replacement pool.",
    },
    state: [
      { label: "Designation", value: sq.designation },
      { label: "Class", value: sq.class },
      { label: "Planes", value: String(sq.planes) },
      { label: "Supplied", value: sq.supplied ? "Yes" : "No — cannot refit (§35.14)" },
    ],
  });
  const unitEntry = (u: UnitView): StackEntry => ({ label: unitLabel(u), detail: unitDetail(u) });
  const sqEntry = (sq: SqLike): StackEntry => ({ label: squadronLabel(sq), detail: squadronDetail(sq) });
  const stackInfo = (title: string, entries: StackEntry[]): InfoPanelData => ({ title, entries });

  return (
    <svg width={w + LEFT_MARGIN + RIGHT_MARGIN} height={h + LABEL_HEADROOM + BOTTOM_MARGIN} viewBox={`${minX - LEFT_MARGIN} ${minY - LABEL_HEADROOM} ${w + LEFT_MARGIN + RIGHT_MARGIN} ${h + LABEL_HEADROOM + BOTTOM_MARGIN}`} style={{ display: "block" }} role="img" aria-label="game board">
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
          : t.kind === "prisoner" ? (t.pr.kind === "Pow" ? t.pr.captor : t.pr.side)
          : t.kind === "captured" ? t.ce.captor
          : t.kind === "dummytank" ? t.dt.side
          : t.kind === "broken" ? t.bv.side
          : ""; // pipeline / airfield: no side

        const side = sideOf(top);
        // Pipelines / airfields aren't side-owned — tint pipe water-blue, airfield slate; else by side.
        const fill = top.kind === "pipe" ? "#4a6b78" : top.kind === "airfield" ? "#556070" : top.kind === "well" ? "#5b6b3a" : top.kind === "ship" && top.sh.wreck ? "#6b6b6b" : side === "Commonwealth" ? "var(--cw)" : "var(--axis)";
        // Status modifiers (broken 🔧 / training 🎓 / dashed border) reflect only the TOP counter's own
        // state — a subordinate tile's status lives in the tooltip, not painted over the representative.
        const topBroken = (top.kind === "unit" && top.u.broken) || (top.kind === "convoy" && top.c.broken > 0);
        const topTraining = (top.kind === "unit" && top.u.in_training) || (top.kind === "rp" && top.r.training_remaining > 0);
        // A 0-TOE HQ (a pure command counter with no organic Strength Points, e.g. the Indian
        // "Headquarters" units) renders like any other unit — solid border/fill, type glyph — but its
        // TOE reads "-" (not applicable). A still-training pool is dashed; a §32.18 dummy dump is
        // deliberately indistinguishable from a real one (only "decoy" gives it away).
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
        const dashed = isTrainingRp || isLooseDump || isHiddenRaider || isProvisionalMine || isUnsuppliedSqn || isWreck;
        const isRommel = top.kind === "rommel";
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
            const kind = t.d.unlimited ? "Supply base (§57)" : t.d.dummy ? "Dummy dump (§32.18)" : t.d.constructed ? "Supply Dump (§24.9)" : "Loose supplies (§24.9 — not convoy-loadable)";
            return `• ${kind} — ${t.d.unlimited ? "unlimited supply" : t.d.dummy ? "no real supplies" : dc}`;
          }
          if (t.kind === "mine") {
            return `• ${t.f.dummy ? "Dummy minefield" : "Minefield"} (§26) — ${t.f.side}${t.f.revealed ? "" : ", unspotted by the enemy"}`;
          }
          if (t.kind === "well")
            return `• Poisoned well (§52.16) — known by ${t.w.cw_knows && t.w.axis_knows ? "both sides" : t.w.cw_knows ? "Commonwealth only" : t.w.axis_knows ? "Axis only" : "neither"}`;
          if (t.kind === "pipe") return `• Water pipeline (§52.2) — unlimited water while connected to a source`;
          if (t.kind === "squadron")
            return `• ${t.sq.designation} squadron (§35) — ${t.sq.side} ${t.sq.class}, ${t.sq.planes} plane(s)${t.sq.supplied ? "" : " — UNSUPPLIED (§35.14)"}`;
          if (t.kind === "cap") return `• Combat Air Patrol (§40) — ${t.cp.side} ${t.cp.cap_type}`;
          if (t.kind === "airfield") return `• ${t.af.kind} (§36) — capacity ${t.af.capacity} squadron(s)`;
          if (t.kind === "prisoner")
            return t.pr.kind === "Pow"
              ? `• Prisoners (§28) — ${t.pr.points} ${t.pr.side} POW point(s), held by ${t.pr.captor}${t.pr.guards > 0 ? `, ${t.pr.guards} guard pt` : " — unguarded"}`
              : `• Escaped prisoners (§28.24) — ${t.pr.points} ${t.pr.side} point(s) heading home`;
          if (t.kind === "captured")
            return `• ${t.ce.destroyed_tank ? "Knocked-out tanks (§14.46)" : "Captured equipment (§28.3)"} — ${t.ce.count}× ${t.ce.nationality} ${t.ce.system}, held by ${t.ce.captor}; ${t.ce.repaired ? "repaired (usable)" : "needs repair"}`;
          if (t.kind === "dummytank")
            return `• ${t.dt.designation} — ${t.dt.side} dummy (decoy) tank marker (§32); indistinguishable from real armour to the enemy`;
          if (t.kind === "broken")
            return `• Broken-down vehicles (§21.5) — ${t.bv.side} (${t.bv.nationality}); ${t.bv.detail}; awaiting tow/repair`;
          // ship
          return t.sh.wreck
            ? `• ${t.sh.name} — scuttled wreck (§55.25), gun ${t.sh.gun}/${t.sh.max_gun}`
            : `• ${t.sh.name} — ${t.sh.side} warship at sea (§30), gun ${t.sh.gun}/${t.sh.max_gun}`;
        };
        // Short list label for a counter (left column of the master-detail panel).
        const shortLabel = (t: Tile): string => {
          if (t.kind === "unit") return t.u.abbr || t.u.name;
          if (t.kind === "rommel") return "Rommel";
          if (t.kind === "raider") return t.k.kind === "Lrdg" ? "LRDG" : "Sonderkommando";
          if (t.kind === "convoy") return `Truck Convoy (${t.c.light + t.c.medium + t.c.heavy} TP)`;
          if (t.kind === "rp") return `${clsShort(t.r.class)} RP ×${t.r.count}`;
          if (t.kind === "dump") return t.d.unlimited ? "Supply base" : t.d.dummy ? "Dummy dump" : t.d.constructed ? "Supply dump" : "Loose supplies";
          if (t.kind === "mine") return t.f.dummy ? "Dummy minefield" : "Minefield";
          if (t.kind === "well") return "Poisoned well";
          if (t.kind === "pipe") return "Water pipeline";
          if (t.kind === "squadron") return `${t.sq.designation.replace(/^(Commonwealth|Axis|Italian|German)\s+/, "")} sqn`;
          if (t.kind === "cap") return "Combat Air Patrol";
          if (t.kind === "airfield") return t.af.kind;
          if (t.kind === "prisoner") return t.pr.kind === "Pow" ? `POW ×${t.pr.points}` : `Escapees ×${t.pr.points}`;
          if (t.kind === "captured") return t.ce.destroyed_tank ? `KO tanks ×${t.ce.count}` : `Captured ×${t.ce.count}`;
          if (t.kind === "dummytank") return t.dt.designation;
          if (t.kind === "broken") return `Broken vehicles ×${t.bv.points}`;
          return t.sh.wreck ? `${t.sh.name} (wreck)` : t.sh.name;
        };
        // Structured detail for a counter's detail pane. Expanded per element type (rules blurb +
        // dynamic state + lore); types not yet expanded fall back to the plain summary line.
        const detailOf = (t: Tile): EntryDetail => {
          if (t.kind === "dump") {
            const d = t.d;
            const rules = {
              description:
                "A supply dump is fuel, ammunition, general stores, and water unloaded into a hex (§24.9). Dumps cannot move and add no stacking cost; a unit in the hex — or tracing supply to it — draws what it needs. An enemy that takes the hex captures the dump intact unless it is destroyed first.",
            };
            let state: StackStateRow[];
            if (d.unlimited) state = [{ label: "Type", value: "Supply base (§57)" }, { label: "Supply", value: "Unlimited" }];
            else if (d.dummy) state = [{ label: "Type", value: "Dummy dump (§32.18)" }, { label: "Contents", value: "Decoy — no real supplies" }];
            else
              state = [
                { label: "Type", value: d.constructed ? "Supply dump" : "Loose supplies" },
                { label: "Fuel", value: String(d.fuel) },
                { label: "Ammo", value: String(d.ammo) },
                { label: "Stores", value: String(d.stores) },
                { label: "Water", value: String(d.water) },
              ];
            return { rules, state };
          }
          if (t.kind === "unit") return unitDetail(t.u);
          if (t.kind === "airfield") {
            const facilityName = (k: string) =>
              k === "AirLandingStrip" ? "Air landing strip"
              : k === "FlyingBoatBasin" ? "Flying-boat basin"
              : k === "FlyingBoatAlightingArea" ? "Flying-boat alighting area"
              : "Airfield";
            const seaplane = t.af.kind.includes("Boat");
            const rules = {
              description: seaplane
                ? "A flying-boat basin/alighting area (§36) bases seaplanes only; land planes cannot use it (§36.3). Capacity limits stationed squadrons (§36.14); the facility can be bombed to reduced capacity or destroyed (0) and rebuilt by engineers."
                : "An airfield or landing strip (§36) bases and services land squadrons. Capacity limits how many squadrons may be stationed (§36.14); it can be bombed down to reduced capacity or destroyed (0), then rebuilt by engineers. Stationed squadrons must be supplied to refit (§35.14).",
            };
            const state: StackStateRow[] = [
              { label: "Facility", value: facilityName(t.af.kind) },
              { label: "Capacity", value: `${t.af.capacity} sqn` },
              { label: "In use", value: `${t.af.occupancy ?? "?"} / ${t.af.capacity}` },
              { label: "Status", value: t.af.capacity === 0 ? "Destroyed" : "Operational" },
            ];
            return { rules, state };
          }
          if (t.kind === "squadron") return squadronDetail(t.sq);
          if (t.kind === "cap") {
            return {
              rules: {
                description:
                  "A Combat Air Patrol (§40.2) is a fighter mission standing over a hex to intercept enemy air activity there, engaging raiders and bombers that enter the covered hex before they can strike.",
              },
              state: [
                { label: "Type", value: t.cp.cap_type },
                { label: "Side", value: t.cp.side },
              ],
            };
          }
          if (t.kind === "convoy") {
            const c = t.c;
            const cargo =
              [c.fuel ? `F${c.fuel}` : "", c.ammo ? `A${c.ammo}` : "", c.stores ? `S${c.stores}` : "", c.water ? `W${c.water}` : ""]
                .filter(Boolean)
                .join(" ") || "empty";
            return {
              rules: {
                description:
                  "A truck convoy (§53) hauls supply and replacements along roads/tracks, spending Truck Points by weight class. Trucks consume fuel and can break down (§21.5); a convoy is captured or destroyed if the enemy catches it in the open.",
              },
              state: [
                { label: "Trucks", value: `${c.light + c.medium + c.heavy} TP (L${c.light} M${c.medium} H${c.heavy})` },
                { label: "Broken", value: c.broken > 0 ? `${c.broken} 🔧` : "none" },
                { label: "Cargo", value: cargo },
                { label: "Replacements", value: c.carries_replacements ? "hauling RP" : "none" },
              ],
            };
          }
          if (t.kind === "rp") {
            const r = t.r;
            return {
              rules: {
                description:
                  "Replacement Points (§20) rebuild understrength units and formations. Commonwealth infantry may arrive green and must train up (§17.3) before use at full value; RP are carried forward by convoy to the units that need them.",
              },
              state: [
                { label: "Class", value: clsShort(r.class) },
                { label: "Count", value: `×${r.count}` },
                { label: "Training", value: r.training_remaining > 0 ? `${r.training_remaining} stage(s) remaining 🎓` : "trained" },
                { label: "Transport", value: r.carried_by != null ? "loaded on convoy" : "awaiting pickup" },
              ],
            };
          }
          if (t.kind === "raider") {
            const k = t.k;
            return {
              rules: {
                description:
                  "A desert raider (§27) — LRDG or Sonderkommando — operates deep behind the lines against supply and airfields. It carries its own fuel and ammo, stays concealed until it acts (§27.32), and draws supply under special rules.",
              },
              state: [
                { label: "Force", value: k.kind === "Lrdg" ? "LRDG" : "Sonderkommando" },
                { label: "Fuel", value: String(k.fuel) },
                { label: "Ammo", value: String(k.ammo) },
                { label: "Concealment", value: k.revealed ? "revealed" : "concealed (§27.32)" },
              ],
            };
          }
          if (t.kind === "mine") {
            const f = t.f;
            return {
              rules: {
                description:
                  "A minefield (§26) impedes movement and can damage units entering it. Its type stays hidden until an enemy tests it (§26.2); a dummy field has no real mines but is indistinguishable until probed.",
              },
              state: [
                { label: "Owner", value: f.side },
                { label: "Type", value: f.dummy ? "Dummy" : "Live" },
                { label: "Spotted", value: f.revealed ? "yes — enemy has tested it" : "no" },
              ],
            };
          }
          if (t.kind === "pipe") {
            return {
              rules: {
                description:
                  "A water pipeline (§52.2) carries unlimited water forward from a source while the line stays connected, supplying units along its hexes and easing the desert water requirement (§51).",
              },
              state: [{ label: "Status", value: "Connected — unlimited water" }],
            };
          }
          if (t.kind === "well") {
            const w = t.w;
            return {
              rules: {
                description:
                  "A village or bir well can be poisoned by any unit for 1 CP (§52.16 — a die roll of 1 salts it). A poisoned well yields no water until sweetened (§52.17). Its state is secret to the enemy until an opposing unit draws there and it is publicly revealed (§52.14); oasis and major-city wells cannot be poisoned.",
              },
              state: [
                { label: "State", value: "Poisoned ☠ (§52.16)" },
                { label: "Commonwealth", value: w.cw_knows ? "knows" : "unaware" },
                { label: "Axis", value: w.axis_knows ? "knows" : "unaware" },
                { label: "Disclosure", value: w.cw_knows && w.axis_knows ? "publicly revealed (§52.14)" : "secret to the enemy" },
              ],
            };
          }
          if (t.kind === "ship") {
            const sh = t.sh;
            return {
              rules: {
                description:
                  "A warship (§30) operates at sea, bombarding coastal hexes and escorting convoys with its Gun Rating. Battle damage reduces its effective gun; a scuttled ship becomes a wreck (§55.25).",
              },
              state: [
                { label: "Name", value: sh.name },
                { label: "Status", value: sh.wreck ? "Scuttled wreck (§55.25)" : "At sea" },
                { label: "Gun", value: `${sh.gun} / ${sh.max_gun}` },
              ],
            };
          }
          if (t.kind === "prisoner") {
            const pr = t.pr;
            return pr.kind === "Pow"
              ? {
                  rules: {
                    description:
                      "Prisoners of war (§28) are enemy strength captured in combat, worth POW points toward victory. They must be guarded (§28.2); unguarded prisoners can escape and convert back into replacement points for their own side.",
                  },
                  state: [
                    { label: "Held by", value: pr.captor },
                    { label: "Points", value: `${pr.points} ${pr.side} POW pt` },
                    { label: "Guards", value: pr.guards > 0 ? `${pr.guards} pt` : "unguarded — may escape" },
                  ],
                }
              : {
                  rules: {
                    description:
                      "Escaped prisoners (§28.24) are POWs that broke free and are making their way home, where they convert back into replacement points for their original side.",
                  },
                  state: [
                    { label: "Side", value: pr.side },
                    { label: "Points", value: `${pr.points} pt` },
                  ],
                };
          }
          if (t.kind === "captured") {
            const ce = t.ce;
            return {
              rules: {
                description:
                  "Captured equipment (§28.3) — enemy kit taken intact — can be repaired and turned against its former owner; knocked-out tanks (§14.46) can likewise be recovered and repaired by either side. It is unusable until repaired.",
              },
              state: [
                { label: "Kind", value: ce.destroyed_tank ? "Knocked-out tanks (§14.46)" : "Captured equipment (§28.3)" },
                { label: "Item", value: `${ce.count}× ${ce.nationality} ${ce.system}` },
                { label: "Held by", value: ce.captor },
                { label: "Condition", value: ce.repaired ? "repaired (usable)" : "needs repair" },
              ],
            };
          }
          if (t.kind === "dummytank") {
            return {
              rules: {
                description:
                  "A dummy (decoy) tank marker (§32) mimics real armour to mislead the enemy, who cannot tell it from a genuine unit until it is engaged or revealed. It has no combat value.",
              },
              state: [
                { label: "Designation", value: t.dt.designation },
                { label: "Side", value: t.dt.side },
                { label: "Nature", value: "Decoy — no combat value" },
              ],
            };
          }
          if (t.kind === "broken") {
            const bv = t.bv;
            return {
              rules: {
                description:
                  "Broken-down vehicles (§21.5) are trucks and weapons immobilized by mechanical failure, left behind until towed or repaired (§22). They can be captured by the enemy if overrun (§21.52).",
              },
              state: [
                { label: "Owner", value: `${bv.side} (${bv.nationality})` },
                { label: "Points", value: String(bv.points) },
                { label: "Detail", value: bv.detail },
                { label: "Status", value: "Awaiting tow/repair" },
              ],
            };
          }
          if (t.kind === "rommel") {
            return {
              rules: {
                description:
                  "Rommel (§31) is a command counter that lends his Command Point Allowance bonus to a unit he joins, boosting its movement and combat. He can shift between units to weight the Axis effort.",
              },
              state: [{ label: "Lending CPA to", value: t.m.attached_to != null ? `unit #${t.m.attached_to}` : "none" }],
            };
          }
          return { summary: line(t).replace(/^•\s*/, "") };
        };
        // Info-panel content for the stack: header line becomes the title; each counter is a
        // selectable entry with a short label and its structured detail.
        const headerSide = side || (top.kind === "pipe" ? "Pipeline" : top.kind === "airfield" ? "Airfield" : "Stack");
        const tileInfo: InfoPanelData = {
          title: `${headerSide} @ (${col},${row})${stack.length > 1 ? ` — ${stack.length} counters` : ""}`,
          entries: sorted.map((t) => ({ label: shortLabel(t), detail: detailOf(t) })),
        };
        // The representative counter's two text rows: a title line and a value/glyph line.
        const body = (() => {
          if (top.kind === "unit") {
            return (
              <>
                <TileTitle text={top.u.abbr || top.u.name} y={-1} size={9} />
                <text x={-9.5} textAnchor="middle" y={8} fontSize={8} fill="#eef">
                  {isCadre ? "-" : top.u.toe}
                </text>
                {top.u.unit_type.length > 0 && <g transform="translate(9.5 5)">{unitTypeGlyph(top.u.unit_type)}</g>}
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
          if (top.kind === "well") {
            return (
              <>
                <text textAnchor="middle" y={-1} fontSize={8.5} fill="#fff" fontWeight={700}>Well</text>
                <text x={-9.5} textAnchor="middle" y={8} fontSize={7} fill="#eef">poison</text>
                {/* a water droplet with a skull cross-out at the glyph slot */}
                <g transform="translate(9.5 5)">
                  <path d="M0,-5 C3,-1 4,1 0,4 C-4,1 -3,-1 0,-5 Z" fill="#cfe6ef" stroke="#0d0f14" strokeWidth={0.5} />
                  <text textAnchor="middle" y={2.5} fontSize={6} fill="#3a2a2a" fontWeight={700}>☠</text>
                </g>
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
                {/* Title = squadron designation; the class is conveyed by the glyph + tooltip. Drop the
                    redundant leading nationality/side word (already shown by tile color) — e.g.
                    "Commonwealth SGSU 51" → "SGSU 51"; a historical name without such a prefix is kept. */}
                <TileTitle text={top.sq.designation.replace(/^(Commonwealth|Axis|Italian|German)\s+/, "")} y={-1} size={8.5} />
                <text x={-9.5} textAnchor="middle" y={8} fontSize={7} fill="#eef">{`x${top.sq.planes}`}</text>
                {/* class-specific top-down plane silhouette (§35.21) */}
                <g transform="translate(9.5 4.5) scale(0.62)" fill="#e9e4cf" stroke="#0d0f14" strokeWidth={0.5}>
                  {squadronGlyph(top.sq.class)}
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
                <TileTitle text={short} y={-1} size={8} />
                <text x={-9.5} textAnchor="middle" y={8} fontSize={7} fill="#eef">{`cap ${top.af.capacity}`}</text>
                {/* §36 paper-map facility symbol (triangle / anchor+triangle / anchor+crossed) */}
                <g transform="translate(9.5 5)">{airfieldGlyph(top.af.kind)}</g>
              </>
            );
          }
          if (top.kind === "prisoner") {
            return (
              <>
                <text textAnchor="middle" y={-1} fontSize={8.5} fill="#fff" fontWeight={700}>{top.pr.kind === "Pow" ? "POW" : "Escape"}</text>
                <text x={-9.5} textAnchor="middle" y={8} fontSize={8} fill="#eef">{`×${top.pr.points}`}</text>
                {/* barbed wire — a strand with X barbs ( -x-x-x- ) at the glyph slot */}
                <g transform="translate(9.5 5)" stroke="#e9e4cf" strokeWidth={1} strokeLinecap="round">
                  <line x1={-6.5} y1={0} x2={6.5} y2={0} />
                  {[-4, 0, 4].map((bx) => (
                    <g key={bx}>
                      <line x1={bx - 1.1} y1={-1.1} x2={bx + 1.1} y2={1.1} />
                      <line x1={bx - 1.1} y1={1.1} x2={bx + 1.1} y2={-1.1} />
                    </g>
                  ))}
                </g>
              </>
            );
          }
          if (top.kind === "captured") {
            return (
              <>
                <TileTitle text={top.ce.destroyed_tank ? "KO Tank" : "Captured"} y={-1} size={8} />
                <text x={-9.5} textAnchor="middle" y={8} fontSize={7} fill="#eef">{`×${top.ce.count}`}</text>
                {/* repaired → check; needs repair → Material Symbols "wrench" (24×24 path, Apache-2.0) */}
                <g transform="translate(9.5 5)">
                  {top.ce.repaired
                    ? <path d="M-4,0 L-1,3 L4,-4" stroke="#e9e4cf" strokeWidth={1} fill="none" strokeLinecap="round" strokeLinejoin="round" />
                    : <path transform="translate(-5 -5) scale(0.42)" fill="#e9e4cf" d="M22.7 19l-9.1-9.1c.9-2.3.4-5-1.5-6.9-2-2-5-2.4-7.4-1.3L9 6 6 9 1.6 4.7C.4 7.1.9 10.1 2.9 12.1c1.9 1.9 4.6 2.4 6.9 1.5l9.1 9.1c.4.4 1 .4 1.4 0l2.3-2.3c.5-.4.5-1.1.1-1.4z" />}
                </g>
              </>
            );
          }
          if (top.kind === "dummytank") {
            // Rendered like an armour counter (a decoy is meant to look real) with a faint "?" cue.
            return (
              <>
                <TileTitle text={top.dt.designation} y={-1} size={8.5} />
                <text x={-9.5} textAnchor="middle" y={8} fontSize={7} fill="#eef">decoy</text>
                <g transform="translate(9.5 5)">{unitTypeGlyph(["Armor"])}</g>
              </>
            );
          }
          if (top.kind === "broken") {
            return (
              <>
                <text textAnchor="middle" y={-1} fontSize={8.5} fill="#fff" fontWeight={700}>Broken</text>
                <text x={-9.5} textAnchor="middle" y={8} fontSize={7} fill="#eef">{`×${top.bv.points}`}</text>
                {/* a broken-down truck (tilted, detached wheel) at the glyph slot */}
                <g transform="translate(9.5 5)" stroke="#e9e4cf" strokeWidth={0.8} fill="none" strokeLinecap="round">
                  <path d="M-5,2 L-5,-2 L1,-2 L3,0 L5,0 L5,2 Z" transform="rotate(-8)" />
                  <circle cx={-3} cy={3.5} r={1.2} />
                  <circle cx={4.5} cy={4} r={1.2} />
                </g>
              </>
            );
          }
          if (top.kind === "ship") {
            return (
              <>
                <TileTitle text={top.sh.wreck ? "Wreck" : top.sh.name} y={-1} size={8} />
                <text x={-9.5} textAnchor="middle" y={8} fontSize={7} fill="#eef">{`⚓${top.sh.gun}`}</text>
                {/* class-specific hull silhouette (BB/CA/CL/CLAA/DD by Gun Rating; wreck lists broken) */}
                <g transform="translate(9.5 5.5) scale(0.9)" fill="#e9e4cf" stroke="#0d0f14" strokeWidth={0.4}>
                  {shipGlyph(top.sh.max_gun, top.sh.wreck)}
                </g>
              </>
            );
          }
          // dump: "Depot" + a 50-gal drum beside a crate. A §32.18 dummy looks identical to a real dump;
          // its value line reads "decoy" where a real one shows contents.
          return (
            <>
              <text textAnchor="middle" y={-1} fontSize={8.5} fill="#fff" fontWeight={700}>{top.d.unlimited ? "Base" : top.d.constructed ? "Depot" : "Cache"}</text>
              <text x={-6} textAnchor="middle" y={8} fontSize={top.d.unlimited ? 11 : 7} fill="#eef">
                {top.d.dummy ? "decoy" : top.d.unlimited ? "∞" : `×${top.d.fuel + top.d.ammo + top.d.stores + top.d.water}`}
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
          <g key={key} transform={`translate(${ctr.x} ${ctr.y})`} {...infoHover(tileInfo)}>
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
            <rect x={px} y={py} width={WPW} height={WPH} rx={6} fill="#e7d9a8" stroke="#b19a55" strokeWidth={1} opacity={0.96} />
            {/* Title + A–E labels explain the weather system and Region bands. */}
            <text x={px + WPW / 2} y={py + 17} textAnchor="middle" fontFamily="Courier New, monospace" fontSize={13} fontWeight={700} fill="#4a3d16" {...infoHover(WEATHER_SYSTEM_INFO)}>
              Weather
            </text>
            {SECTION_CENTERS.map((_, i) => {
              const cx = px + 8 + cellW * (i + 0.5);
              const cond = weather?.[i] ?? "Normal";
              const condInfo = WEATHER_CONDITION_INFO[cond] ?? WEATHER_CONDITION_INFO.Normal;
              return (
                <g key={i}>
                  <text x={cx} y={py + 36} textAnchor="middle" fontFamily="Courier New, monospace" fontSize={12} fontWeight={700} fill="#6b5a2e" {...infoHover(WEATHER_SYSTEM_INFO)}>
                    {String.fromCharCode(65 + i)}
                  </text>
                  {/* The glyph + condition name explain the current condition's effects. */}
                  <g transform={`translate(${cx} ${py + 58})`}>{weatherGlyph(cond)}</g>
                  <text x={cx} y={py + 92} textAnchor="middle" fontFamily="Courier New, monospace" fontSize={8.5} fill="#6b5a2e">
                    {cond}
                  </text>
                  {/* transparent bounding box over the glyph + name so the condition tooltip triggers
                      anywhere in the cell, not just on the icon strokes */}
                  <rect x={cx - cellW / 2} y={py + 46} width={cellW} height={50} fill="transparent" {...infoHover(condInfo)} />
                </g>
              );
            })}
          </g>
        );
      })()}
      {/* Replacement-Point delivery: a dashed line from each assigned on-map pool toward its unit's
          hex (§20.41/§20.49). The pools themselves render as counters in the hex stack above; staged
          future arrivals (§20.21) are listed in a panel. */}
      {(() => {
        const reps = replacements ?? [];
        const unitHex = new Map<number, [number, number]>();
        for (const u of units) if (u.hex) unitHex.set(u.id, u.hex as [number, number]);
        const onMap = reps.filter((r) => r.hex);
        const staged = reps.filter((r) => !r.hex);
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
            <g {...infoHover({ title: "Incoming Replacements (§20)", rules: { description: "Replacement Points rebuild units' losses (§20). Produced points are scheduled and arrive after a planning delay (Axis 2 / Commonwealth 4 Game-Turns, §20.21), then must Train (§17.3/§20.43) before they can absorb into a unit. This panel lists points still in transit — their arrival Game-Turn, side, class, and count — before they appear on the map." } })}>
              <rect x={panelX - 234} y={panelY} width={220} height={28 + Math.max(1, staged.length) * 15} rx={6} fill="#e7d9a8" stroke="#b19a55" strokeWidth={1} opacity={0.96} />
              <text x={panelX - 124} y={panelY + 17} textAnchor="middle" fontFamily="Courier New, monospace" fontSize={13} fontWeight={700} fill="#4a3d16">Incoming Replacements</text>
              {staged.length > 0 ? staged.map((r, i) => (
                <text key={i} x={panelX - 224} y={panelY + 34 + i * 15} fontFamily="Courier New, monospace" fontSize={10} fill="#6b5a2e">
                  {`GT ${r.arrival ? `${r.arrival[0]}/${r.arrival[1]}` : "?"} — ${r.side.slice(0, 2)} ${r.class} ×${r.count}`}
                </text>
              )) : (
                <text x={panelX - 124} y={panelY + 34} textAnchor="middle" fontFamily="Courier New, monospace" fontSize={10} fontStyle="italic" fill="#8a7a3a">None pending</text>
              )}
            </g>
          </g>
        );
      })()}
      {/* Naval panels (§55/§56): a Ports box (efficiencies) and, below it, an Axis Convoys box —
          abstract/named naval state that has no map hex, so it lives in off-map panels. */}
      {naval && (naval.ports.length > 0 || naval.convoys.length > 0) && (() => {
        const boxW = 372;
        const px = panelX - boxW - 14;
        const py = panelY + 150;
        const portsH = 42 + naval.ports.length * 14;
        const convoyY = py + (naval.ports.length > 0 ? portsH + 10 : 0);
        return (
          <g>
            {naval.ports.length > 0 && (
              <g>
                <rect x={px} y={py} width={boxW} height={portsH} rx={6} fill="#e7d9a8" stroke="#b19a55" strokeWidth={1} opacity={0.96} />
                <text x={px + boxW / 2} y={py + 17} textAnchor="middle" fontFamily="Courier New, monospace" fontSize={13} fontWeight={700} fill="#4a3d16" {...infoHover(PORT_SYSTEM_INFO)}>Ports</text>
                {/* section column headers */}
                <text x={px + 170} y={py + 30} textAnchor="middle" fontFamily="Courier New, monospace" fontSize={8} fontWeight={700} fill="#6b5a2e">Shipping Tonnage</text>
                <text x={px + 305} y={py + 30} textAnchor="middle" fontFamily="Courier New, monospace" fontSize={8} fontWeight={700} fill="#6b5a2e">Troop Transfer</text>
                {naval.ports.map((p, i) => (
                  <g key={`np${i}`}>
                    <text x={px + 10} y={py + 48 + i * 14} fontFamily="Courier New, monospace" fontSize={10} fill={p.blocked ? "#a33" : "#4a3f22"}>
                      <tspan fontWeight={700}>{p.name}</tspan>
                      <tspan x={px + 96}>{`${p.efficiency}/${p.max_efficiency}`}</tspan>
                      <tspan x={px + 132}>{`→ ${p.throughput}/${p.max_tonnage}t`}</tspan>
                      <tspan x={px + 250}>{`⇅ ${p.stacking_in ?? "-"}/${p.stacking_out ?? "-"} SP${p.blocked ? " ⚠" : ""}`}</tspan>
                    </text>
                    {/* §30.5/§55.13: checked once this port has transferred troops this Operations Stage. */}
                    <g transform={`translate(${px + boxW - 16} ${py + 44 + i * 14})`}>
                      <rect x={-5} y={-5} width={11} height={11} rx={1.5} fill={p.transferred_this_stage ? "#4a3f22" : "#efe2b3"} stroke="#4a3f22" strokeWidth={1} />
                      {p.transferred_this_stage && <path d="M-3,0 L-1,2.3 L3.5,-3" stroke="#fff" strokeWidth={1.5} fill="none" strokeLinecap="round" strokeLinejoin="round" />}
                    </g>
                  </g>
                ))}
                {/* transparent bounding-box hit areas so each column's tooltip triggers consistently
                    across its whole zone (name → general, tonnage columns, transfer columns). */}
                {(() => {
                  const zTop = py + 23, zBot = py + portsH - 5;
                  const zone = (x1: number, x2: number, info: InfoPanelData) => (
                    <rect x={x1} y={zTop} width={x2 - x1} height={zBot - zTop} fill="transparent" {...infoHover(info)} />
                  );
                  return (
                    <>
                      {zone(px + 6, px + 92, PORT_SYSTEM_INFO)}
                      {zone(px + 92, px + 244, PORT_TONNAGE_INFO)}
                      {zone(px + 244, px + boxW - 4, PORT_TRANSFER_INFO)}
                    </>
                  );
                })()}
              </g>
            )}
            {/* Always shown (a stable panel), with a placeholder when no convoy is in transit. */}
            <g>
              <rect x={px} y={convoyY} width={boxW} height={28 + Math.max(1, naval.convoys.length) * 14} rx={6} fill="#e7d9a8" stroke="#b19a55" strokeWidth={1} opacity={0.96} />
              <text x={px + boxW / 2} y={convoyY + 17} textAnchor="middle" fontFamily="Courier New, monospace" fontSize={13} fontWeight={700} fill="#4a3d16" {...infoHover(CONVOY_SYSTEM_INFO)}>Axis Convoys</text>
              {naval.convoys.length > 0 ? (
                naval.convoys.map((c, i) => (
                  <text key={`nc${i}`} x={px + 10} y={convoyY + 34 + i * 14} fontFamily="Courier New, monospace" fontSize={10} fill="#4a3f22">
                    {`GT${c.arrival_gt} L${c.lane}→${c.destination} ${c.tons}t`}
                  </text>
                ))
              ) : (
                <text x={px + 10} y={convoyY + 34} fontFamily="Courier New, monospace" fontSize={10} fontStyle="italic" fill="#8a7a3a">None scheduled</text>
              )}
            </g>
          </g>
        );
      })()}
      {/* §8.8 Tripoli/Tunisia off-map complex: a horizontal strip in the sea WEST of the map (the
          real direction of these places). Reads map→inland left as: [Nofilia junction] · Tripolitania
          · Tripoli · Gabes · Tunis — i.e. Tripolitania nearest the map on the right, Tunis farthest
          on the left (the reverse of the paper counter-sheet, which is laid out to fit its box). Each
          location box shows its name, AA, and port; between boxes are the fast/slow transit tracks
          (§8.89). Units rest in boxes or sit on a track cell at their transit step. */}
      {offMap && offMap.nodes.length > 0 && (() => {
        // Normalise with per-node defaults (from off-map-nodes.ron) so an older game.jsonl lacking
        // the leg/AA/port fields still lays out correctly — notably Tunis→Gabes is a 5-stage slow leg.
        const PORT: Record<string, string> = { Tripoli: "Tripoli", Tunis: "Bizerta" };
        const MID: Record<string, number> = { Tripolitania: 1, Tripoli: 1, Gabes: 1, Tunis: 2 };
        const SLOW: Record<string, number> = { Tripolitania: 3, Tripoli: 3, Gabes: 3, Tunis: 5 };
        const nodes = offMap.nodes.map((n) => ({
          ...n,
          aa_points: n.aa_points ?? 0,
          leg_fast: n.leg_fast || 1,
          leg_mid: n.leg_mid || MID[n.name] || 1,
          leg_slow: n.leg_slow || SLOW[n.name] || 3,
          port: n.port ?? PORT[n.name] ?? null,
        }));
        const boxW = 84, boxH = 100, cellW = 42, cGap = 3, gap = 9, nofW = 40;
        // A leg of N stages has N−1 in-transit cells (the last stage arrives in the next box).
        const trackW = (slow: number) => Math.max(1, slow - 1) * (cellW + cGap) - cGap;
        // Right edge just west of the map; lay columns out right→left.
        const rightX = minX - 16;
        const topY = hexCenter(3, 27).y - boxH / 2;
        let x = rightX;
        const nofX = x - nofW; x = nofX - gap;
        const trackX: number[] = [], boxX: number[] = [];
        nodes.forEach((n, k) => {
          const tw = trackW(n.leg_slow);
          trackX[k] = x - tw; x = trackX[k] - gap;
          boxX[k] = x - boxW; x = boxX[k] - gap;
        });
        const leftX = x + gap;
        // Tolerate an older game.jsonl whose off-map units predate the embedded full `unit`: fall back
        // to a minimal counter synthesized from the legacy name/abbr/side fields (TOE/glyph blank).
        const unitOf = (u: OffMapUnitView): UnitView => {
          if (u.unit) return u.unit;
          const l = u as unknown as { name?: string; abbr?: string; side?: string };
          return { id: 0, name: l.name ?? "", abbr: l.abbr ?? l.name ?? "", nationality: "", side: l.side ?? "Axis", hex: null, org_size: "", unit_type: [], toe: 0, fuel: 0, ammo: 0, cohesion: 0, basic_morale: 0, morale_reference: 0, in_training: false, broken: false, is_hq: false } as unknown as UnitView;
        };
        return (
          <g>
            {/* parent background box holding the whole complex (as with the Malta box) */}
            <rect x={leftX - 12} y={topY - 30} width={rightX - leftX + 24} height={boxH + 44} rx={7} fill="#e7d9a8" stroke="#b19a55" strokeWidth={1} />
            <text x={(leftX + rightX) / 2} y={topY - 14} textAnchor="middle" fontFamily="Courier New, monospace" fontSize={12} fontWeight={700} fill="#3a2f14" {...infoHover(OFF_MAP_INFO)}>Axis Off-Map Land Movement (§8.8)</text>
            {/* Nofilia map-junction marker (rightmost, adjacent to the board). */}
            <g>
              <rect x={nofX} y={topY} width={nofW} height={boxH} rx={4} fill="#dfe7ef" stroke="#8aa0b4" strokeWidth={1} opacity={0.95} />
              <text x={nofX + nofW / 2} y={topY + boxH / 2 - 4} textAnchor="middle" fontFamily="Courier New, monospace" fontSize={8} fontWeight={700} fill="#33465a">Nofilia</text>
              <text x={nofX + nofW / 2} y={topY + boxH / 2 + 7} textAnchor="middle" fontFamily="Courier New, monospace" fontSize={7} fill="#55697d">A2802 →</text>
            </g>
            {nodes.map((n, k) => {
              // Transit track between box k and the box one step closer to the map — three lanes for
              // the three CPA tiers (§8.89). A leg of N stages has N−1 *in-transit* cells (the Nth
              // stage arrives in the next box): fast (CPA ≥25) and medium (CPA 15–24) are one stage on
              // every leg except Tunis→Gabes (medium = 2), so they show a straight arrow; slow
              // (CPA <15) shows its leg−1 step boxes. Cells are 42 wide; lanes are 22 tall.
              const laneH = 22;
              const cellsOf = (n_: number) => Math.max(0, n_ - 1);
              const slowCells = cellsOf(n.leg_slow);
              const tw = Math.max(1, slowCells) * (cellW + cGap) - cGap;
              const fastY = topY + 18, medY = topY + 44, slowY = topY + 70;
              const cellCX = (idx: number) => trackX[k] + idx * (cellW + cGap) + cellW / 2;
              const onTrack = n.units.filter((u) => u.transit > 0 && u.toward_map);
              // A rightward (toward-map) arrow from x1 to a head at x2.
              const arrowSeg = (x1: number, x2: number, y: number) => (
                <g stroke="#b19a55" strokeWidth={1} fill="#b19a55">
                  <line x1={x1} y1={y} x2={x2 - 2} y2={y} />
                  <path d={`M${x2 - 6},${y - 3} L${x2},${y} L${x2 - 6},${y + 3} Z`} stroke="none" />
                </g>
              );
              // A single arrow spanning the whole track, for a lane with no step boxes.
              const arrow = (y: number) => arrowSeg(trackX[k], trackX[k] + tw, y);
              // One lane: `cells` step boxes (a single box centered; several left-aligned), or an arrow.
              const renderLane = (cells: number, laneTop: number, label: string) => (
                <g>
                  <text x={trackX[k]} y={laneTop - 2} fontFamily="Courier New, monospace" fontSize={5.5} fill="#8a7a4a">{label}</text>
                  {cells === 0
                    ? arrow(laneTop + laneH / 2)
                    : cells === 1
                    ? (() => {
                        // A single centered step box with a separate arrow on each side (location →
                        // box → next), each with its own head and `gap`-sized spaces at the box and ends.
                        const cy = laneTop + laneH / 2;
                        const bl = trackX[k] + tw / 2 - cellW / 2, br = trackX[k] + tw / 2 + cellW / 2;
                        return (
                          <g>
                            {arrowSeg(trackX[k], bl - gap, cy)}
                            <rect x={bl} y={laneTop} width={cellW} height={laneH} rx={2} fill="none" stroke="#c3aa63" strokeWidth={0.8} strokeDasharray="2 2" />
                            {arrowSeg(br + gap, trackX[k] + tw, cy)}
                          </g>
                        );
                      })()
                    : Array.from({ length: cells }).map((_, ci) => (
                        <rect key={ci} x={trackX[k] + ci * (cellW + cGap)} y={laneTop} width={cellW} height={laneH} rx={2} fill="none" stroke="#c3aa63" strokeWidth={0.8} strokeDasharray="2 2" />
                      ))}
                </g>
              );
              return (
                <g key={n.name}>
                  {/* fast/medium/slow transit channels — one hoverable group with a bounding hit-area */}
                  <g {...infoHover(OFF_MAP_TRANSIT_INFO)}>
                    <rect x={trackX[k] - 2} y={fastY - 8} width={Math.max(tw, cellW) + 4} height={slowY + laneH - fastY + 12} fill="transparent" />
                    {renderLane(cellsOf(n.leg_fast), fastY, "fast")}
                    {renderLane(cellsOf(n.leg_mid), medY, "medium")}
                    {renderLane(slowCells, slowY, "slow")}
                  </g>
                  {onTrack.map((u, ui) => {
                    // Which lane: slow tier, else medium tier, else fast (fast/medium 1-stage legs
                    // never hold an in-transit unit — they arrive the same stage).
                    const [laneTop, centered] = u.leg_total >= n.leg_slow ? [slowY, false] : u.leg_total >= n.leg_mid ? [medY, true] : [fastY, true];
                    const idx = Math.min(Math.max(0, u.transit - 1), Math.max(0, slowCells - 1));
                    const cx = centered ? trackX[k] + tw / 2 : cellCX(idx);
                    const uv = unitOf(u);
                    return (
                      <g
                        key={`${n.name}-t${ui}`}
                        transform={`translate(${cx} ${(laneTop as number) + laneH / 2})`}
                        {...infoHover(stackInfo(`${n.name} — in transit (§8.89)`, [unitEntry(uv)]))}
                      >
                        <UnitCounter u={uv} noTitle />
                      </g>
                    );
                  })}
                  {/* location box — lighter inner shade (as with the Malta airfield cells) */}
                  <rect x={boxX[k]} y={topY} width={boxW} height={boxH} rx={4} fill="#efe2b3" stroke="#c3aa63" strokeWidth={1} />
                  <text x={boxX[k] + boxW / 2} y={topY + 13} textAnchor="middle" fontFamily="Courier New, monospace" fontSize={9} fontWeight={700} fill="#4a3d16" {...(loreOfPlace(n.name) ? infoHover({ title: n.name, lore: loreOfPlace(n.name) }) : {})}>{n.name}</text>
                  <text x={boxX[k] + boxW / 2} y={topY + 24} textAnchor="middle" fontFamily="Courier New, monospace" fontSize={7.5} fill="#6b5a2e">
                    <tspan {...infoHover(OFF_MAP_AA_INFO)}>{`AA ${n.aa_points}`}</tspan>
                    {n.port && <tspan {...infoHover(OFF_MAP_PORT_INFO)}>{` · ⚓${n.port}`}</tspan>}
                  </text>
                  {/* §20.43: Tripoli is the Axis off-map training area — the map training-ground glyph. */}
                  {n.name === "Tripoli" && (
                    <g transform={`translate(${boxX[k] + boxW / 2} ${topY + 28}) scale(0.8)`} {...infoHover({ title: "Axis Training Area (§20.43)", rules: { description: "Axis Replacement Points train here (§20.43 — Tripoli or any major city). Training runs for the §20.43 schedule of Operations Stages with an instruction battalion present at Basic Morale (§17.35)." } })}>
                      <rect x={-8} y={-6} width={16} height={30} fill="transparent" />
                      {featureGlyph("AxisTrainingArea")}
                    </g>
                  )}
                  {/* resting units (transit 0): representative tile + badge */}
                  {(() => {
                    const rest = n.units.filter((u) => u.transit === 0);
                    if (rest.length === 0) return <text x={boxX[k] + boxW / 2} y={topY + boxH / 2 + 16} textAnchor="middle" fontFamily="Courier New, monospace" fontSize={7.5} fontStyle="italic" fill="#9a8a4a">— empty —</text>;
                    return (
                      <g
                        transform={`translate(${boxX[k] + boxW / 2} ${topY + boxH / 2 + 16})`}
                        {...infoHover(stackInfo(`${n.name} — off-map box`, rest.map((r) => unitEntry(unitOf(r)))))}
                      >
                        <UnitCounter u={unitOf(rest[0])} extra={rest.length - 1} noTitle />
                      </g>
                    );
                  })()}
                </g>
              );
            })}
          </g>
        );
      })()}
      {/* §43 Axis Mediterranean bomber bases: a manila swatch per off-map box holding a stack of
          squadron counters. Italy/Sicily above Region A, Crete above the top-left of Section C
          (roughly their paper-map positions). Empty until the Luftwaffe arrives (GT26+). */}
      {(air?.med_bases?.length ? air.med_bases : [{ name: "Italy/Sicily", squadrons: [] }, { name: "Crete", squadrons: [] }]).map((mb) => {
        const bw = 152;
        const bh = 60;
        const cx = mb.name === "Crete" ? hexCenter(72, 1).x : hexCenter(17, 1).x;
        const bx = cx - bw / 2;
        // Crete sits in the top margin; Italy/Sicily drops below the "Region A" label (44px, baseline
        // ~minY-12) so it clears it.
        const by = mb.name === "Crete" ? minY - 46 : minY + 2;
        const shown = mb.squadrons.slice(0, 5);
        const extra = mb.squadrons.length - shown.length;
        const title = `Axis ${mb.name.replace("/", " & ")} Air Bases`;
        return (
          <g key={mb.name}>
            <rect x={bx} y={by} width={bw} height={bh} rx={5} fill="#e7d9a8" stroke="#b19a55" strokeWidth={1} opacity={0.97} />
            <text x={bx + bw / 2} y={by + 15} textAnchor="middle" fontFamily="Courier New, monospace" fontSize={Math.min(10, (bw - 12) / (title.length * 0.6))} fontWeight={700} fill="#4a3d16" {...infoHover(MED_BASE_INFO)}>{title}</text>
            {shown.length === 0 ? (
              <text x={bx + bw / 2} y={by + 40} textAnchor="middle" fontFamily="Courier New, monospace" fontSize={9} fontStyle="italic" fill="#8a7a3a">— no squadrons —</text>
            ) : (
              shown.map((s, i) => (
                <g key={i} transform={`translate(${bx + 22 + i * 17} ${by + 38})`}>
                  <rect x={-11} y={-8} width={22} height={16} rx={2} fill="var(--axis)" stroke="#0d0f14" strokeWidth={0.7} />
                  <g transform="translate(0 -1) scale(0.5)" fill="#e9e4cf" stroke="#0d0f14" strokeWidth={0.6}>{squadronGlyph(s.class)}</g>
                  <text x={0} y={7} textAnchor="middle" fontSize={5} fill="#fff">{`x${s.planes}`}</text>
                </g>
              ))
            )}
            {extra > 0 && <text x={bx + bw - 6} y={by + 42} textAnchor="end" fontFamily="Courier New, monospace" fontSize={9} fontWeight={700} fill="#4a3d16">{`+${extra}`}</text>}
          </g>
        );
      })}
      {/* §44 Malta box — the air facilities (Takali/Luqa/Safi/Hal Far airfields, Kalafrana basin,
          Valetta alighting area) with their §36 kind, live capacity level, based-plane count
          (§44.14, capacity = level × 18), and any diverted AA (§44.18). Driven by the `air.malta`
          delta layer; falls back to the static roster before the first AirView.
          Positioned in the sea between Sicily and the African coast. */}
      {(() => {
        type Group = { aircraft_type: string; class: string; planes: number };
        type Field = { name: string; kind: string; levels: number; maxLevels: number; planes: number; flakLight: number; flakHeavy: number; groups: Group[] };
        // Live facilities when available, else the static §44.1 seed roster (empty, pre-play).
        const fields: Field[] = air?.malta?.length
          ? air.malta.map((b) => ({ name: b.name, kind: b.kind, levels: b.levels, maxLevels: b.max_levels, planes: b.planes, flakLight: b.flak_light, flakHeavy: b.flak_heavy, groups: b.groups ?? [] }))
          : ([
              ["Takali", "Airfield", 6], ["Luqa", "Airfield", 6], ["Safi", "Airfield", 6],
              ["Hal Far", "Airfield", 6], ["Kalafrana", "FlyingBoatBasin", 3], ["Valetta", "FlyingBoatAlightingArea", 1],
            ] as [string, string, number][]).map(([name, kind, maxLevels]) => ({ name, kind, levels: maxLevels, maxLevels, planes: 0, flakLight: 0, flakHeavy: 0, groups: [] }));
        const maltaAaTotal = fields.reduce((s, f) => s + f.flakLight + f.flakHeavy, 0);
        // Diverted flak (§44.18) is the AA that fires against a raid; every field also has 1 intrinsic
        // AA point (§36.18), shown as "+1i", usable only vs strafing/dive-bombing (not the level-bombing
        // Malta raid). Heavy diverted flak is annotated (nH) — it can't be fighter-suppressed (§40.74).
        const flakLabel = (l: number, h: number) => {
          const d = l + h;
          return `AA: ${d}${h > 0 ? `(${h}H)` : ""} + 1`;
        };
        // A rectangular grid of airfield cells inside the parent Malta box.
        const cols = 3;
        const rows = Math.ceil(fields.length / cols);
        const cellW = 122, cellH = 76, gapX = 6, gapY = 6, pad = 8, titleH = 22;
        const gridW = cols * cellW + (cols - 1) * gapX;
        const bw = gridW + 2 * pad;
        const bh = titleH + rows * cellH + (rows - 1) * gapY + pad;
        // Just right of the Italy/Sicily Med box (centre hexCenter(17,1).x, width 152, top minY+2),
        // tops aligned, with a 12px gutter.
        const bx = hexCenter(17, 1).x + 76 + 12;
        const by = minY + 2;
        // Historical-context lore (from public/lore/places.json) for the island and each field, shown
        // as the panel's lore section alongside the rules text.
        const loreOf = (key: string) => {
          const e = placeLore?.[key] as { description?: string; links?: { label: string; url: string }[] } | undefined;
          return e?.description ? { description: e.description, links: e.links } : undefined;
        };
        return (
          <g>
            <rect x={bx} y={by} width={bw} height={bh} rx={5} fill="#e7d9a8" stroke="#b19a55" strokeWidth={1} opacity={0.97} />
            <text x={bx + bw / 2} y={by + 15} textAnchor="middle" fontFamily="Courier New, monospace" fontSize={11} fontWeight={700} fill="#4a3d16" {...infoHover({ ...MALTA_INFO, lore: loreOf("Malta") })}>{maltaAaTotal > 0 ? `Commonwealth Malta Air Bases · AA ${maltaAaTotal}/48` : "Commonwealth Malta Air Bases"}</text>
            {fields.map((f, i) => {
              const cx = bx + pad + (i % cols) * (cellW + gapX);
              const cy = by + titleH + Math.floor(i / cols) * (cellH + gapY);
              const cap = f.levels * 18;
              const rep = f.groups[0];
              const extra = Math.max(0, f.groups.length - 1);
              return (
                <g key={f.name}>
                  <rect x={cx} y={cy} width={cellW} height={cellH} rx={4} fill="#efe2b3" stroke="#c3aa63" strokeWidth={1} />
                  {/* type glyph (hoverable rules helper) + field name */}
                  <g transform={`translate(${cx + 16} ${cy + 14}) scale(1.3)`} {...infoHover(facilityInfo(f.kind))}>
                    {/* transparent hit-area so the whole glyph box is hoverable, not just the strokes */}
                    <rect x={-8} y={-6} width={16} height={13} fill="transparent" />
                    {airfieldGlyph(f.kind, "#5a4a1e")}
                  </g>
                  <text x={cx + cellW / 2} y={cy + 14} textAnchor="middle" fontFamily="Courier New, monospace" fontSize={9} fontWeight={700} fill="#4a3d16" {...(loreOf(f.name) ? infoHover({ title: `${f.name} — Malta`, lore: loreOf(f.name), rules: facilityInfo(f.kind).rules }) : {})}>{f.name}</text>
                  {/* capacity/usage (planes) — its own rules helper (§44.14) */}
                  <text x={cx + cellW / 2} y={cy + 28} textAnchor="middle" fontFamily="Courier New, monospace" fontSize={8.5} fill="#4a3f22" {...infoHover(MALTA_CAPACITY_INFO)}><tspan fontSize={14} dx={2} dy={2}>✈</tspan><tspan dy={-2}>{` ${f.planes}/${cap}`}</tspan></text>
                  {/* AA / flak — its own rules helper (§44.18) */}
                  <text x={cx + cellW / 2} y={cy + 40} textAnchor="middle" fontFamily="Courier New, monospace" fontSize={8.5} fill="#4a3f22" {...infoHover(MALTA_AA_INFO)}>{flakLabel(f.flakLight, f.flakHeavy)}</text>
                  {/* stationed planes as a squadron counter stack (rep + badge, on-map logic) */}
                  {rep ? (
                    <g
                      transform={`translate(${cx + cellW / 2} ${cy + 58})`}
                      {...infoHover(stackInfo(`${f.name} — Malta`, f.groups.map((gr) => sqEntry({ designation: gr.aircraft_type, class: gr.class, planes: gr.planes, supplied: true }))))}
                    >
                      <SquadronCounter designation={rep.aircraft_type} planes={rep.planes} cls={rep.class} side="Commonwealth" extra={extra} noTitle />
                    </g>
                  ) : (
                    <text x={cx + cellW / 2} y={cy + 60} textAnchor="middle" fontFamily="Courier New, monospace" fontSize={8} fontStyle="italic" fill="#9a8a4a">— empty —</text>
                  )}
                </g>
              );
            })}
          </g>
        );
      })()}
      {/* §36.53 Commonwealth off-map air facilities — one manila box per Canal-zone box + Ethiopia,
          in a horizontal strip across the top of Region E, just below the "Region E" label. Each box
          shows the field, its on-map entry hex, and its squadron/plane occupancy. */}
      {(() => {
        // Live facilities when available, else the static §36.53 roster so the strip renders pre-play.
        const STATIC: NonNullable<AirView["cw_off_map"]> = [
          { name: "Port Said", entry_hex: "E4033", max_squadrons: 3, flying_boat: true, squadrons: [] },
          { name: "Abu Seier", entry_hex: "E3433", max_squadrons: 9, flying_boat: false, squadrons: [] },
          { name: "Ismailia", entry_hex: "E3433", max_squadrons: 9, flying_boat: false, squadrons: [] },
          { name: "Fayid", entry_hex: "E1833", max_squadrons: 9, flying_boat: false, squadrons: [] },
          { name: "Deversoir", entry_hex: "E1833", max_squadrons: 9, flying_boat: false, squadrons: [] },
          { name: "Kabrit", entry_hex: "E1833", max_squadrons: 9, flying_boat: false, squadrons: [] },
          { name: "Ethiopia", entry_hex: "E0127", max_squadrons: null, flying_boat: false, squadrons: [] },
        ];
        const list = air?.cw_off_map?.length ? air.cw_off_map : STATIC;
        // Standalone cells modeled on the Malta airfield cells (no parent box): kind glyph + name,
        // a squadron capacity line (no AA — these off-map fields have none), and a stationed-squadron
        // counter stack.
        const bw = 104, bh = 74, gap = 8;
        // The six Canal-zone boxes sit east of the map (right margin) near their entry-hex rows; the
        // Ethiopia box sits below the map (its real direction, south). Each has a dashed connector to
        // the entry hex. `conn` = {ax,ay} box attach point → {hx,hy} hex-edge point.
        const boxLeftX = minX + w + 8;
        const belowMapY = minY + h + 18;
        const anchorOf = (f: (typeof list)[number]) => {
          const p = parseHexId(f.entry_hex);
          return p ? hexCenter(p.col, p.row) : null;
        };
        // Canal-zone boxes packed top→bottom in the right margin so ones sharing an entry hex
        // (Fayid/Deversoir/Kabrit at E1833; Abu Seier/Ismailia at E3433) stack without overlapping.
        const canal = list.filter((f) => f.name !== "Ethiopia").map((f) => ({ f, anchor: anchorOf(f) }))
          .sort((a, b) => (a.anchor?.y ?? 0) - (b.anchor?.y ?? 0));
        let cursorY = -Infinity;
        const placed = canal.map(({ f, anchor }) => {
          const by = Math.max((anchor?.y ?? minY) - bh / 2, cursorY);
          cursorY = by + bh + gap;
          return { f, bx: boxLeftX, by, conn: anchor ? { ax: boxLeftX, ay: by + bh / 2, hx: anchor.x + 22, hy: anchor.y } : null };
        });
        // Ethiopia below the map, centered under its entry hex (clamped within the map's x-span).
        const eth = list.find((f) => f.name === "Ethiopia");
        if (eth) {
          const a = anchorOf(eth);
          const bx = Math.min(Math.max((a?.x ?? minX + w / 2) - bw / 2, minX), minX + w - bw);
          placed.push({ f: eth, bx, by: belowMapY, conn: a ? { ax: bx + bw / 2, ay: belowMapY, hx: a.x, hy: a.y + 26 } : null });
        }
        return (
          <g>
            {placed.map(({ f, bx, by, conn }) => {
              const used = f.squadrons.length;
              const cap = f.max_squadrons == null ? "∞" : String(f.max_squadrons);
              const kind = f.flying_boat ? "FlyingBoatBasin" : "Airfield";
              const rep = f.squadrons[0];
              return (
                <g key={f.name}>
                  {/* dashed connector from the box to the entry hex */}
                  {conn && (
                    <g stroke="#8a7a3a" strokeWidth={1.2} fill="#8a7a3a">
                      <line x1={conn.hx} y1={conn.hy} x2={conn.ax} y2={conn.ay} strokeDasharray="4 3" />
                      <circle cx={conn.hx} cy={conn.hy} r={2.2} stroke="none" />
                    </g>
                  )}
                  <rect x={bx} y={by} width={bw} height={bh} rx={4} fill="#efe2b3" stroke="#c3aa63" strokeWidth={1} />
                  {/* type glyph (hoverable rules helper) + field name */}
                  <g transform={`translate(${bx + 16} ${by + 14}) scale(1.3)`} {...infoHover(facilityInfo(kind))}>
                    <rect x={-8} y={-6} width={16} height={13} fill="transparent" />
                    {airfieldGlyph(kind, "#5a4a1e")}
                  </g>
                  <text x={bx + bw / 2} y={by + 14} textAnchor="middle" fontFamily="Courier New, monospace" fontSize={9} fontWeight={700} fill="#4a3d16" {...(loreOfPlace(f.name) ? infoHover({ title: f.name, lore: loreOfPlace(f.name) }) : {})}>{f.name}</text>
                  {/* capacity/usage in squadrons (no AA) */}
                  <text x={bx + bw / 2} y={by + 28} textAnchor="middle" fontFamily="Courier New, monospace" fontSize={8.5} fill="#4a3f22" {...infoHover(CW_OFF_MAP_INFO)}>{`${used}/${cap} sq`}</text>
                  {/* stationed squadrons — representative counter + badge (on-map logic); the tooltip
                      lists the whole stack, not just the top counter */}
                  {rep ? (
                    <g
                      transform={`translate(${bx + bw / 2} ${by + 50})`}
                      {...infoHover(stackInfo(f.name, f.squadrons.map((s) => sqEntry({ designation: s.designation, class: s.class, planes: s.planes, supplied: s.supplied }))))}
                    >
                      <SquadronCounter designation={rep.designation} planes={rep.planes} cls={rep.class} side="Commonwealth" supplied={rep.supplied} extra={f.squadrons.length - 1} noTitle />
                    </g>
                  ) : (
                    <text x={bx + bw / 2} y={by + 52} textAnchor="middle" fontFamily="Courier New, monospace" fontSize={8} fontStyle="italic" fill="#9a8a4a">— empty —</text>
                  )}
                </g>
              );
            })}
          </g>
        );
      })()}
      {/* Legend inset drawn in map coordinates (foreignObject) so it pans/zooms with the board.
          Terrain Key (+ compass) on the left; Unit Key to its right. */}
      {(legend || unitKey) && (
        <foreignObject x={legendX} y={legendY} width={LW * 2} height={LH} style={{ overflow: "visible" }}>
          <div style={{ display: "flex", gap: 12, alignItems: "flex-start" }}>
            <div style={{ display: "flex", flexDirection: "column", gap: 12, alignItems: "flex-start" }}>
              {legend}
            </div>
            {unitKey}
          </div>
        </foreignObject>
      )}
    </svg>
  );
});
