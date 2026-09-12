import { useEffect, useMemo, useRef, useState } from "react";
import type { GameRecord } from "@bindings/GameRecord";
import type { UnitView } from "@bindings/UnitView";
import { mapBounds } from "./hex";
import { HexMap } from "./components/HexMap";
import { Scrubber } from "./components/Scrubber";
import { ActionLog } from "./components/ActionLog";
import { TerrainKey } from "./components/TerrainKey";
import { LEVELS, buildNodes, nodeIndexOf, type Level, type Node } from "./hierarchy";

export function App() {
  const [record, setRecord] = useState<GameRecord | null>(null);
  const [error, setError] = useState<string | null>(null);
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
          <HexMap map={record.map} units={frames[idx] ?? []} weather={weatherFrames[idx]} legend={<TerrainKey />} />
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
    </div>
  );
}
