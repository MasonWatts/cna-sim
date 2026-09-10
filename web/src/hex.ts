// Hex geometry, mirroring the engine (crates/engine/src/state/map.rs): pointy-top, even-r offset.
// We replicate `to_cube` then map cube → pixel so on-screen adjacency matches the engine exactly.

export const HEX_SIZE = 26; // circumradius in px

interface Cube {
  x: number;
  z: number;
}

// even-r offset (col, row) → cube. Matches HexCoord::to_cube. `row + (row & 1)` is always even,
// so the halving is exact.
function toCube(col: number, row: number): Cube {
  const z = row;
  const x = col - (row + (row & 1)) / 2;
  return { x, z };
}

/** Pixel center of hex (col,row) for pointy-top layout, before view offset. Row increases *upward*
 *  (the engine's (1,1) is the lower-left tile), so y is negated — SVG y grows downward. */
export function hexCenter(col: number, row: number): { x: number; y: number } {
  const { x, z } = toCube(col, row);
  return {
    x: HEX_SIZE * Math.sqrt(3) * (x + z / 2),
    y: -HEX_SIZE * 1.5 * z,
  };
}

/** SVG polygon points for a pointy-top hexagon centered at (cx, cy). */
export function hexPolygon(cx: number, cy: number, size = HEX_SIZE): string {
  const pts: string[] = [];
  for (let i = 0; i < 6; i++) {
    const angle = (Math.PI / 180) * (60 * i - 30); // pointy-top: vertex at top
    pts.push(`${(cx + size * Math.cos(angle)).toFixed(2)},${(cy + size * Math.sin(angle)).toFixed(2)}`);
  }
  return pts.join(" ");
}
