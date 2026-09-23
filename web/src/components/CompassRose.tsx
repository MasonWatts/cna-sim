// A cosmetic compass rose for the map legend, modeled on the paper map's rose: a fat eight-ray
// asterisk star with a small separate north-arrow triangle above the "N". The board is pointy-top
// with rows increasing upward (see hex.ts / map.rs), so North is up, East right. Drawn in the paper
// legend's ink on the cream legend background. The map's large bounding circle is intentionally
// omitted.

const INK = "#3a352b";

export function CompassRose({ size = 74 }: { size?: number }) {
  const cx = size / 2;
  // Nudge the drawing origin down within the square viewBox so "S" ends up a small pad above the
  // bottom edge (the north triangle still has ample room above).
  const cy = size / 2 + size * 0.12;
  // Point in the direction `deg` (0 = up/North, clockwise) at radius `r`, as an [x,y] pair.
  const pt = (deg: number, r: number): [number, number] => {
    const a = (deg * Math.PI) / 180;
    return [cx + r * Math.sin(a), cy - r * Math.cos(a)];
  };
  const xy = ([x, y]: [number, number]) => `${x.toFixed(2)},${y.toFixed(2)}`;

  // The star is four straight rectangular bars crossing through the centre at 0/45/90/135°, giving
  // eight equal spokes that overlap in the middle (constant width — not tapered).
  const R = size * 0.42; // half-length of each bar (spoke length from centre)
  const hw = size * 0.07; // half-width of each bar
  const bar = (deg: number) => {
    const a = (deg * Math.PI) / 180;
    const ux = Math.sin(a), uy = -Math.cos(a); // along the bar (0 = up)
    const px = Math.cos(a), py = Math.sin(a); // across the bar
    const corner = (su: number, sp: number): [number, number] => [cx + su * R * ux + sp * hw * px, cy + su * R * uy + sp * hw * py];
    return `${xy(corner(1, 1))} ${xy(corner(1, -1))} ${xy(corner(-1, -1))} ${xy(corner(-1, 1))}`;
  };
  const rays = [0, 45, 90, 135].map((deg) => (
    <polygon key={deg} points={bar(deg)} fill={INK} stroke={INK} strokeWidth={0.6} strokeLinejoin="round" />
  ));

  const lr = size * 0.62; // label radius (beyond the spoke tips so labels stay legible)
  const label = (deg: number, text: string, fontFactor = 0.34) => {
    const [x, y] = pt(deg, lr);
    return (
      <text x={x} y={y} fill={INK} fontSize={size * fontFactor} fontWeight={700} textAnchor="middle" dominantBaseline="central">
        {text}
      </text>
    );
  };

  // North-arrow triangle, apex up, sitting above the (now larger) "N" label.
  const triApex = pt(0, size * 1.0);
  const triL = pt(-6, size * 0.86);
  const triR = pt(6, size * 0.86);

  const pad = size * 0.46; // room for the larger labels and the north triangle beyond the star
  return (
    <svg
      width={size}
      height={size}
      viewBox={`${-pad} ${-pad} ${size + 2 * pad} ${size + 2 * pad}`}
      aria-label="Compass rose: north is up"
      role="img"
      style={{ display: "block" }}
    >
      {rays}
      <polygon points={`${xy(triApex)} ${xy(triL)} ${xy(triR)}`} fill={INK} stroke={INK} strokeWidth={0.6} strokeLinejoin="round" />
      {label(0, "N", 0.46)}
      {label(90, "E")}
      {label(180, "S")}
      {label(270, "W")}
    </svg>
  );
}
