import { useEffect, useRef } from "react";
import type { Event } from "@bindings/Event";

/** Scrollable list of every recorded action; the current step is highlighted and scrolled into view. */
export function ActionLog({
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

  return (
    <ul className="log" style={{ maxHeight: "40vh", overflow: "auto" }}>
      {events.map((e, i) => (
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
      ))}
    </ul>
  );
}
