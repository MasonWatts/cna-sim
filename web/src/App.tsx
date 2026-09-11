import { useEffect, useMemo, useRef, useState } from "react";
import type { GameRecord } from "@bindings/GameRecord";
import type { UnitView } from "@bindings/UnitView";
import { mapBounds } from "./hex";
import { HexMap } from "./components/HexMap";
import { Scrubber } from "./components/Scrubber";
import { ActionLog } from "./components/ActionLog";

export function App() {
  const [record, setRecord] = useState<GameRecord | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [step, setStep] = useState(0);
  const [playing, setPlaying] = useState(false);
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

  // Auto-advance while playing; stop at the end.
  useEffect(() => {
    if (!playing) return;
    timer.current = window.setInterval(() => {
      setStep((s) => {
        if (s >= lastStep) {
          setPlaying(false);
          return s;
        }
        return s + 1;
      });
    }, 250);
    return () => {
      if (timer.current) window.clearInterval(timer.current);
    };
  }, [playing, lastStep]);

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

  const idx = Math.min(step, lastStep);
  const event = record?.events[idx];

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
        <span className="meta">
          Turn {event.game_turn} · Stage {event.operations_stage} · {event.phase}
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
          <HexMap map={record.map} units={frames[idx] ?? []} />
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
        onStep={setStep}
        onTogglePlay={() => setPlaying((p) => !p)}
      />
    </div>
  );
}
