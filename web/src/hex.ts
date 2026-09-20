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

/** Parse a hex label like `"E1833"` (section letter A–E + two-digit RR row + two-digit CC col) into
 *  global (col,row), mirroring the engine's `HexId::to_hex_coord` (col = sectionIndex*33 + CC). */
export function parseHexId(s: string): { col: number; row: number } | null {
  const m = /^([A-Ea-e])(\d\d)(\d\d)$/.exec(s.trim());
  if (!m) return null;
  const idx = m[1].toUpperCase().charCodeAt(0) - 65;
  return { col: idx * 33 + Number(m[3]), row: Number(m[2]) };
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

/** Pixel bounding box (content coordinates) covering all hexes, with a hex of padding. */
export function mapBounds(hexes: { col: number; row: number }[]): { minX: number; minY: number; w: number; h: number } {
  const pad = HEX_SIZE * 1.6;
  let minX = Infinity, minY = Infinity, maxX = -Infinity, maxY = -Infinity;
  for (const hex of hexes) {
    const c = hexCenter(hex.col, hex.row);
    if (c.x < minX) minX = c.x;
    if (c.y < minY) minY = c.y;
    if (c.x > maxX) maxX = c.x;
    if (c.y > maxY) maxY = c.y;
  }
  return { minX: minX - pad, minY: minY - pad, w: maxX - minX + 2 * pad, h: maxY - minY + 2 * pad };
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
