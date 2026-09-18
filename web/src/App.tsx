import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { loadLore, type InfoPanelData, type LoreMap } from "./lore";
import { InfoPanel } from "./components/InfoPanel";
import type { GameRecord } from "@bindings/GameRecord";
import type { UnitView } from "@bindings/UnitView";
import type { ReplacementView } from "@bindings/ReplacementView";
import type { SupplyDumpView } from "@bindings/SupplyDumpView";
import type { TruckConvoyView } from "@bindings/TruckConvoyView";
import type { RaiderView } from "@bindings/RaiderView";
import type { RommelView } from "@bindings/RommelView";
import type { MapOverlaysView } from "@bindings/MapOverlaysView";
import type { HexsideView } from "@bindings/HexsideView";
import type { AirView } from "@bindings/AirView";
import type { NavalView } from "@bindings/NavalView";
import { mapBounds } from "./hex";
import { HexMap } from "./components/HexMap";
import { Scrubber } from "./components/Scrubber";
import { ActionLog } from "./components/ActionLog";
import { TerrainKey } from "./components/TerrainKey";
import { UnitKey } from "./components/UnitKey";
import { CompassRose } from "./components/CompassRose";
import { LEVELS, buildNodes, nodeIndexOf, type Level, type Node } from "./hierarchy";

export function App() {
  const [record, setRecord] = useState<GameRecord | null>(null);
  const [error, setError] = useState<string | null>(null);
  // Historical-context ("lore") panel: dataset loaded from /lore/places.json; `hover` holds the entry
  // being shown and where. A short hide delay lets the cursor travel into the panel to click links.
  const [placeLore, setPlaceLore] = useState<LoreMap>({});
  const [info, setInfo] = useState<{ data: InfoPanelData; x: number; y: number } | null>(null);
  const hideTimer = useRef<number | null>(null);
  const [step, setStep] = useState(0);
  const [playing, setPlaying] = useState(false);
  const [level, setLevel] = useState<Level>("week");
  const [zoom, setZoom] = useState(1);
  const [pan, setPan] = useState({ x: 0, y: 0 });
  const [dragging, setDragging] = useState(false);
  const timer = useRef<number | null>(null);
  const mapWrap = useRef<HTMLDivElement>(null);
  const zoomRef = useRef(1);
  const panRef = useRef({ x: 0, y: 0 });
  const dragLast = useRef<{ x: number; y: number } | null>(null);
  zoomRef.current = zoom;
  panRef.current = pan;

  const clampZ = (z: number) => Math.max(0.02, Math.min(6, z));

  // Set zoom to `nz` keeping the content point under pane-local pixel (cx,cy) fixed (zoom-to-cursor).
  const zoomAbout = (nz: number, cx: number, cy: number) => {
    const z = zoomRef.current;
    const p = panRef.current;
    nz = clampZ(nz);
    setPan({ x: cx - (cx - p.x) * (nz / z), y: cy - (cy - p.y) * (nz / z) });
    setZoom(nz);
  };
  const zoomAtCenter = (nz: number) => {
    const el = mapWrap.current;
    zoomAbout(nz, (el?.clientWidth ?? 0) / 2, (el?.clientHeight ?? 0) / 2);
  };

  // Fit + center the map in the pane.
  const fit = () => {
    if (!record || !mapWrap.current) return;
    const b = mapBounds(record.map.hexes);
    const el = mapWrap.current;
    const z = clampZ(Math.min(el.clientWidth / b.w, el.clientHeight / b.h));
    setZoom(z);
    setPan({ x: (el.clientWidth - b.w * z) / 2, y: (el.clientHeight - b.h * z) / 2 });
  };

  // Scroll-wheel zoom toward the cursor (native listener so we can preventDefault the page scroll).
  useEffect(() => {
    const el = mapWrap.current;
    if (!el) return;
    const onWheel = (e: WheelEvent) => {
      e.preventDefault();
      const rect = el.getBoundingClientRect();
      const factor = e.deltaY < 0 ? 1.15 : 1 / 1.15;
      zoomAbout(zoomRef.current * factor, e.clientX - rect.left, e.clientY - rect.top);
    };
    el.addEventListener("wheel", onWheel, { passive: false });
    return () => el.removeEventListener("wheel", onWheel);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [record]);

  // Click-drag to pan.
  const onDown = (e: React.MouseEvent) => {
    setDragging(true);
    dragLast.current = { x: e.clientX, y: e.clientY };
  };
  const onMove = (e: React.MouseEvent) => {
    if (!dragLast.current) return;
    const dx = e.clientX - dragLast.current.x;
    const dy = e.clientY - dragLast.current.y;
    dragLast.current = { x: e.clientX, y: e.clientY };
    setPan((p) => ({ x: p.x + dx, y: p.y + dy }));
  };
  const onUp = () => {
    setDragging(false);
    dragLast.current = null;
  };

  // Load the recorded game. Prefer the streamed JSONL log (game.jsonl: a header line then one event
  // per line); fall back to the single-document game.json.
  useEffect(() => {
    const parseJsonl = (text: string): GameRecord => {
      const lines = text.split("\n").filter((l) => l.trim().length > 0);
      const header = JSON.parse(lines[0]);
      const events = lines.slice(1).map((l) => JSON.parse(l));
      return { ...header, events } as GameRecord;
    };
    fetch("/game.jsonl")
      .then((r) => (r.ok ? r.text().then(parseJsonl) : Promise.reject(new Error("no jsonl"))))
      .catch(() =>
        fetch("/game.json").then((r) =>
          r.ok ? (r.json() as Promise<GameRecord>) : Promise.reject(new Error(`HTTP ${r.status}`)),
        ),
      )
      .then((r: GameRecord) => setRecord(r))
      .catch((e) => setError(String(e)));
  }, []);

  // Load the place-lore dataset once (graceful empty on failure).
  useEffect(() => {
    loadLore("places").then(setPlaceLore);
  }, []);

  const cancelHide = useCallback(() => {
    if (hideTimer.current) {
      window.clearTimeout(hideTimer.current);
      hideTimer.current = null;
    }
  }, []);
  // Generic info-panel show/hide (used by place labels, terrain-key entries, …).
  const showInfo = useCallback(
    (data: InfoPanelData, x: number, y: number) => {
      cancelHide();
      setInfo({ data, x, y });
    },
    [cancelHide],
  );
  const hideInfo = useCallback(() => {
    cancelHide();
    hideTimer.current = window.setTimeout(() => setInfo(null), 160);
  }, [cancelHide]);
  // Place labels carry lore only (no rules).
  const onPlaceHover = useCallback(
    (name: string, x: number, y: number) => {
      const entry = placeLore[name];
      if (!entry) return;
      showInfo({ title: entry.title ?? name, lore: { description: entry.description, links: entry.links } }, x, y);
    },
    [placeLore, showInfo],
  );

  const lastStep = record ? record.events.length - 1 : 0;

  // Auto-advance while playing, one node at the current granularity per tick; stop at the end.
  // Coarser levels tick slower so each month/week is legible.
  useEffect(() => {
    if (!playing) return;
    timer.current = window.setInterval(
      () => {
        setStep((s) => {
          const n = nextAt(level, s);
          if (n <= s) {
            setPlaying(false);
            return s;
          }
          return n;
        });
      },
      level === "step" ? 200 : 700,
    );
    return () => {
      if (timer.current) window.clearInterval(timer.current);
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [playing, lastStep, level]);

  // Fit the map to the pane once it loads (a 7k-hex map would otherwise render at full pixel size).
  useEffect(() => {
    if (record) fit();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [record]);

  // The event log stores unit state as deltas; fold it into a full unit array per step once on load.
  const frames = useMemo<UnitView[][]>(() => {
    if (!record) return [];
    const out: UnitView[][] = [];
    const cur = new Map<number, UnitView>();
    for (const e of record.events) {
      for (const u of e.units) cur.set(u.id, u);
      for (const r of e.removed) cur.delete(r);
      out.push([...cur.values()]);
    }
    return out;
  }, [record]);

  // Per-section weather is a delta too; fold it forward into a full [A,B,C,D,E] array per step.
  const weatherFrames = useMemo<string[][]>(() => {
    if (!record) return [];
    const out: string[][] = [];
    let cur: string[] = ["Normal", "Normal", "Normal", "Normal", "Normal"];
    for (const e of record.events) {
      if (e.weather) cur = e.weather;
      out.push(cur);
    }
    return out;
  }, [record]);

  // Replacement-Point pools (§20) are a delta too; fold forward into a full array per step.
  const replacementFrames = useMemo<ReplacementView[][]>(() => {
    if (!record) return [];
    const out: ReplacementView[][] = [];
    let cur: ReplacementView[] = [];
    for (const e of record.events) {
      if (e.replacements) cur = e.replacements;
      out.push(cur);
    }
    return out;
  }, [record]);

  // Supply Dumps (§24.9) are a delta too; fold forward into a full array per step.
  const supplyDumpFrames = useMemo<SupplyDumpView[][]>(() => {
    if (!record) return [];
    const out: SupplyDumpView[][] = [];
    let cur: SupplyDumpView[] = [];
    for (const e of record.events) {
      if (e.supply_dumps) cur = e.supply_dumps;
      out.push(cur);
    }
    return out;
  }, [record]);

  // Free truck convoys (§53) are a delta too; fold forward into a full array per step.
  const truckConvoyFrames = useMemo<TruckConvoyView[][]>(() => {
    if (!record) return [];
    const out: TruckConvoyView[][] = [];
    let cur: TruckConvoyView[] = [];
    for (const e of record.events) {
      if (e.truck_convoys) cur = e.truck_convoys;
      out.push(cur);
    }
    return out;
  }, [record]);

  // Desert raiders (§27) and the Rommel counter (§31) are deltas too; fold forward per step.
  const raiderFrames = useMemo<RaiderView[][]>(() => {
    if (!record) return [];
    const out: RaiderView[][] = [];
    let cur: RaiderView[] = [];
    for (const e of record.events) {
      if (e.raiders) cur = e.raiders;
      out.push(cur);
    }
    return out;
  }, [record]);
  const rommelFrames = useMemo<RommelView[][]>(() => {
    if (!record) return [];
    const out: RommelView[][] = [];
    let cur: RommelView[] = [];
    for (const e of record.events) {
      if (e.rommel) cur = e.rommel;
      out.push(cur);
    }
    return out;
  }, [record]);

  // Map overlays (minefields / forts / pipelines, §26/§24.4/§52.2) — one delta object; fold forward.
  const overlayFrames = useMemo<MapOverlaysView[]>(() => {
    if (!record) return [];
    const out: MapOverlaysView[] = [];
    let cur: MapOverlaysView = { minefields: [], fortifications: [], pipelines: [] };
    for (const e of record.events) {
      if (e.overlays) cur = e.overlays;
      out.push(cur);
    }
    return out;
  }, [record]);

  // Hexside features (routes + edge terrain) change during play (rail/road built or destroyed §24.6/
  // §24.66); fold the delta forward, starting from the header's initial map.
  const hexsideFrames = useMemo<HexsideView[][]>(() => {
    if (!record) return [];
    const out: HexsideView[][] = [];
    let cur: HexsideView[] = record.map.hexsides ?? [];
    for (const e of record.events) {
      if (e.hexsides) cur = e.hexsides;
      out.push(cur);
    }
    return out;
  }, [record]);

  // Air layer (facilities / squadrons / CAP patrols, §35/§36/§40) — one delta object; fold forward.
  const airFrames = useMemo<AirView[]>(() => {
    if (!record) return [];
    const out: AirView[] = [];
    let cur: AirView = { facilities: [], squadrons: [], caps: [] };
    for (const e of record.events) {
      if (e.air) cur = e.air;
      out.push(cur);
    }
    return out;
  }, [record]);

  // Naval layer (ships / ports / convoys, §30/§55/§56) — one delta object; fold forward.
  const navalFrames = useMemo<NavalView[]>(() => {
    if (!record) return [];
    const out: NavalView[] = [];
    let cur: NavalView = { ships: [], ports: [], convoys: [] };
    for (const e of record.events) {
      if (e.naval) cur = e.naval;
      out.push(cur);
    }
    return out;
  }, [record]);

  // Node structures for each grouping level (Month/Week/Op-Stage/Phase), for level-aware navigation.
  const nodesByLevel = useMemo<Record<Exclude<Level, "step">, Node[]>>(
    () => ({
      week: buildNodes(record?.events ?? [], "week"),
      stage: buildNodes(record?.events ?? [], "stage"),
      phase: buildNodes(record?.events ?? [], "phase"),
    }),
    [record],
  );

  // Next/previous event index at a given granularity: step → ±1; else the next/previous node start.
  const nextAt = (lvl: Level, s: number): number => {
    if (lvl === "step") return Math.min(s + 1, lastStep);
    const nodes = nodesByLevel[lvl];
    if (!nodes.length) return lastStep;
    const ni = nodeIndexOf(nodes, s);
    return ni + 1 < nodes.length ? nodes[ni + 1].start : lastStep;
  };
  const prevAt = (lvl: Level, s: number): number => {
    if (lvl === "step") return Math.max(s - 1, 0);
    const nodes = nodesByLevel[lvl];
    if (!nodes.length) return 0;
    const ni = nodeIndexOf(nodes, s);
    return s > nodes[ni].start ? nodes[ni].start : nodes[Math.max(ni - 1, 0)].start;
  };

  const idx = Math.min(step, lastStep);
  const event = record?.events[idx];
  const curNodes = level === "step" ? [] : nodesByLevel[level];
  const nodeIndex = level === "step" ? idx : nodeIndexOf(curNodes, idx);
  const nodeTotal = level === "step" ? lastStep + 1 : curNodes.length;
  const curNodeLabel = level === "step" ? `step ${idx}` : (curNodes[nodeIndex]?.label ?? "");

  if (error) return <div style={{ padding: 24 }}>Failed to load <code>game.json</code>: {error}</div>;
  if (!record || !event) return <div style={{ padding: 24 }}>Loading…</div>;

  return (
    <div className="app">
      <div className="header">
        <h1>CNA-Sim — {record.scenario}</h1>
        <span className="meta">
          <span className="chip cw">CW: {record.commonwealth_agent}</span>{" "}
          <span className="chip axis">Axis: {record.axis_agent}</span>
        </span>
        <span className="meta breadcrumb" title="Click a level to set playback granularity">
          {LEVELS.filter((l) => l !== "step").map((l, i) => {
            const nodes = nodesByLevel[l as Exclude<Level, "step">];
            const lbl = nodes[nodeIndexOf(nodes, idx)]?.label ?? "";
            return (
              <span key={l}>
                {i > 0 && " › "}
                <button className={`crumb ${l === level ? "active" : ""}`} onClick={() => setLevel(l)}>
                  {lbl}
                </button>
              </span>
            );
          })}
          {" › "}
          <button className={`crumb ${level === "step" ? "active" : ""}`} onClick={() => setLevel("step")}>
            step {event.step}
          </button>
        </span>
        <span className="meta zoom">
          <button onClick={() => zoomAtCenter(zoom / 1.25)} title="Zoom out">−</button>
          <input
            type="range"
            min={0.05}
            max={4}
            step={0.01}
            value={zoom}
            onChange={(e) => zoomAtCenter(Number(e.target.value))}
            aria-label="zoom"
          />
          <button onClick={() => zoomAtCenter(zoom * 1.25)} title="Zoom in">+</button>
          <button onClick={fit} title="Fit map to window">Fit</button>
          <span style={{ minWidth: 42, textAlign: "right" }}>{Math.round(zoom * 100)}%</span>
        </span>
      </div>

      <div
        className="map-wrap"
        ref={mapWrap}
        onMouseDown={onDown}
        onMouseMove={onMove}
        onMouseUp={onUp}
        onMouseLeave={onUp}
        style={{ cursor: dragging ? "grabbing" : "grab" }}
        title="Scroll to zoom · drag to pan"
      >
        <div style={{ transform: `translate(${pan.x}px, ${pan.y}px) scale(${zoom})`, transformOrigin: "0 0", width: "max-content" }}>
          {/* Variant C is drawn inside HexMap in map coordinates (see `legend`), so it pans/zooms
              with the board and sits off-shore at the map's horizontal center. */}
          <HexMap map={record.map} units={frames[idx] ?? []} weather={weatherFrames[idx]} replacements={replacementFrames[idx] ?? []} supplyDumps={supplyDumpFrames[idx] ?? []} truckConvoys={truckConvoyFrames[idx] ?? []} raiders={raiderFrames[idx] ?? []} rommel={rommelFrames[idx] ?? []} overlays={overlayFrames[idx]} hexsides={hexsideFrames[idx]} air={airFrames[idx]} naval={navalFrames[idx]} placeLore={placeLore} onPlaceHover={onPlaceHover} onPlaceLeave={hideInfo} onInfoHover={showInfo} legend={<>
              <div className="legend-map"><TerrainKey onInfoHover={showInfo} onInfoLeave={hideInfo} /></div>
              <div className="legend-map compass-box"><div className="tk-title">Orientation</div><CompassRose /></div>
            </>}
            unitKey={<div className="legend-map"><UnitKey unitTypes={record.unit_types} /></div>} />
        </div>
      </div>

      <div className="sidebar">
        <div className="panel-section">
          <h3>Current action</h3>
          {event.side !== "-" && <span className={`chip ${event.side === "Commonwealth" ? "cw" : "axis"}`}>{event.side}</span>}
          <div style={{ marginTop: 6 }}>
            <code>{event.action}</code>
          </div>
          {event.outcome && (
            <div style={{ marginTop: 6, color: "var(--muted)" }}>
              → <code>{event.outcome}</code>
            </div>
          )}
        </div>
        <div className="panel-section">
          <h3>Relevant rules</h3>
          <div style={{ color: "var(--muted)" }}>
            {event.rules.length ? event.rules.join(", ") : "— (per-action § citations pending)"}
          </div>
        </div>
        <div className="panel-section">
          <h3>Action log</h3>
          <ActionLog events={record.events} current={step} onSelect={setStep} />
        </div>
      </div>

      <Scrubber
        step={step}
        last={lastStep}
        playing={playing}
        level={level}
        nodeIndex={nodeIndex}
        nodeTotal={nodeTotal}
        nodeLabel={curNodeLabel}
        onLevel={setLevel}
        onPrev={() => setStep(prevAt(level, step))}
        onNext={() => setStep(nextAt(level, step))}
        onSlider={(v) => setStep(level === "step" ? v : (curNodes[v]?.start ?? 0))}
        onRestart={() => setStep(0)}
        onTogglePlay={() => setPlaying((p) => !p)}
      />

      {/* Historical-context panel: a stylized floating tooltip shown while hovering a place label
          (or the panel itself, so its "read more" links stay clickable). Content comes from the
          lore dataset — see src/lore.ts / public/lore/places.json. */}
      {info && (
        <div
          className="lore-panel"
          style={{ left: Math.min(info.x, window.innerWidth - 340), top: Math.min(info.y + 8, window.innerHeight - 180) }}
          onMouseEnter={cancelHide}
          onMouseLeave={hideInfo}
        >
          <InfoPanel {...info.data} />
        </div>
      )}
    </div>
  );
}
