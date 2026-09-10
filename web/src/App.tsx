import { useEffect, useMemo, useRef, useState } from "react";
import type { GameRecord } from "@bindings/GameRecord";
import type { UnitView } from "@bindings/UnitView";
import { HexMap } from "./components/HexMap";
import { Scrubber } from "./components/Scrubber";
import { ActionLog } from "./components/ActionLog";

export function App() {
  const [record, setRecord] = useState<GameRecord | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [step, setStep] = useState(0);
  const [playing, setPlaying] = useState(false);
  const timer = useRef<number | null>(null);

  // Load the recorded game.
  useEffect(() => {
    fetch("/game.json")
      .then((r) => (r.ok ? r.json() : Promise.reject(new Error(`HTTP ${r.status}`))))
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
      </div>

      <div className="map-wrap">
        <HexMap map={record.map} units={frames[idx] ?? []} />
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
