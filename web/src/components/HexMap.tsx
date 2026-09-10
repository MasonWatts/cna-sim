import type { MapView } from "@bindings/MapView";
import type { UnitView } from "@bindings/UnitView";
import { HEX_SIZE, hexCenter, hexPolygon } from "../hex";

const TERRAIN_FILL: Record<string, string> = {
  Clear: "#2a2f1c",
  Desert: "#3a3320",
  Sand: "#3a3320",
  Rough: "#3a2f26",
  Mountain: "#3b3540",
  Escarpment: "#332a2a",
  Salt: "#26323a",
  Sea: "#1a2b3a",
  Oasis: "#1f3a2a",
};
const terrainFill = (t: string) => TERRAIN_FILL[t] ?? "#242a1e";

/** The board at one step: static terrain hexes + the units currently on them. */
export function HexMap({ map, units }: { map: MapView; units: UnitView[] }) {
  // View bounds from all hex centers.
  const centers = map.hexes.map((h) => hexCenter(h.col, h.row));
  const pad = HEX_SIZE * 1.6;
  const xs = centers.map((c) => c.x);
  const ys = centers.map((c) => c.y);
  const minX = Math.min(...xs) - pad;
  const minY = Math.min(...ys) - pad;
  const w = Math.max(...xs) - minX + pad;
  const h = Math.max(...ys) - minY + pad;

  // Group units by hex so we can fan out stacks.
  const byHex = new Map<string, UnitView[]>();
  for (const u of units) {
    if (!u.hex) continue;
    const key = `${u.hex[0]},${u.hex[1]}`;
    (byHex.get(key) ?? byHex.set(key, []).get(key)!).push(u);
  }

  return (
    <svg width={w} height={h} viewBox={`${minX} ${minY} ${w} ${h}`} role="img" aria-label="game board">
      {/* terrain */}
      {map.hexes.map((hex) => {
        const c = hexCenter(hex.col, hex.row);
        return (
          <polygon
            key={`${hex.col},${hex.row}`}
            points={hexPolygon(c.x, c.y)}
            fill={terrainFill(hex.terrain)}
            stroke="#0d0f14"
            strokeWidth={1}
          >
            <title>{`${hex.terrain} (${hex.col},${hex.row})`}</title>
          </polygon>
        );
      })}
      {/* units */}
      {[...byHex.entries()].map(([key, stack]) => {
        const [col, row] = key.split(",").map(Number);
        const c = hexCenter(col, row);
        return stack.map((u, i) => {
          // fan a stack around the hex center
          const n = stack.length;
          const offset = n === 1 ? { x: 0, y: 0 } : { x: (i - (n - 1) / 2) * 12, y: (i - (n - 1) / 2) * 6 };
          const fill = u.side === "Commonwealth" ? "var(--cw)" : "var(--axis)";
          const cx = c.x + offset.x;
          const cy = c.y + offset.y;
          return (
            <g key={u.id} transform={`translate(${cx} ${cy})`}>
              <rect
                x={-15}
                y={-11}
                width={30}
                height={22}
                rx={3}
                fill={fill}
                stroke={u.broken ? "#e7b53a" : "#0d0f14"}
                strokeWidth={u.broken ? 2 : 1}
              />
              <text textAnchor="middle" y={-1} fontSize={9} fill="#fff" fontWeight={700}>
                {u.name.slice(0, 7)}
              </text>
              <text textAnchor="middle" y={8} fontSize={8} fill="#eef">
                {u.toe}
              </text>
              {/* §21 broken-down marker: amber wrench glyph in the corner. */}
              {u.broken && (
                <text x={11} y={-4} fontSize={11} textAnchor="middle" aria-label="broken down">
                  🔧
                </text>
              )}
              <title>{`${u.name} — ${u.side}${u.broken ? " ⚠ BROKEN DOWN (§21)" : ""}\nTOE ${u.toe}  fuel ${u.fuel}  ammo ${u.ammo}  cohesion ${u.cohesion}\n${u.org_size} @ (${col},${row})`}</title>
            </g>
          );
        });
      })}
    </svg>
  );
}
