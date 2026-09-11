import { memo, useEffect, useRef } from "react";
import type { Event } from "@bindings/Event";

const WINDOW = 150; // rows rendered on each side of the current step

/**
 * Scrollable list of nearby actions; the current step is highlighted and scrolled into view.
 * Perf: `memo`'d (so pan/zoom skip it — only `current` changes it) and windowed to ±`WINDOW` rows
 * around the current step, so scrubbing never rebuilds the whole (tens-of-thousands-long) log.
 */
export const ActionLog = memo(function ActionLog({
  events,
  current,
  onSelect,
}: {
  events: Event[];
  current: number;
  onSelect: (step: number) => void;
}) {
  const activeRef = useRef<HTMLLIElement | null>(null);
  useEffect(() => {
    activeRef.current?.scrollIntoView({ block: "nearest" });
  }, [current]);

  // A compact one-line label from the Debug string (drop the struct-field noise for the list).
  const short = (action: string) => action.split(/[{(]/)[0].trim() || action;

  const start = Math.max(0, current - WINDOW);
  const end = Math.min(events.length, current + WINDOW);

  return (
    <ul className="log" style={{ maxHeight: "40vh", overflow: "auto" }}>
      {start > 0 && <li className="meta">… {start} earlier</li>}
      {events.slice(start, end).map((e, j) => {
        const i = start + j;
        return (
          <li
            key={i}
            ref={i === current ? activeRef : undefined}
            className={i === current ? "active" : ""}
            onClick={() => onSelect(i)}
            title={e.action}
          >
            <span className="meta">{e.step}. </span>
            {e.side !== "-" && <span className={`chip ${e.side === "Commonwealth" ? "cw" : "axis"}`}>{e.side[0]}</span>}{" "}
            <span className="act">{short(e.action)}</span>
          </li>
        );
      })}
      {end < events.length && <li className="meta">… {events.length - end} more</li>}
    </ul>
  );
});
