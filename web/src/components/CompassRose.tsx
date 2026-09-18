// A cosmetic compass rose for the map legend. The board is pointy-top with rows increasing upward
// (see hex.ts / map.rs), so North is straight up, East right — a standard orientation. Drawn in the
// paper-legend ink/cream palette so it sits naturally beside the Terrain Key.

const INK = "#3a352b";
const CREAM = "#efe8c6";

export function CompassRose({ size = 74 }: { size?: number }) {
  const c = size / 2;
  const hub = size * 0.05; // radius the star points spring from
  // Point in the direction `deg` (0 = up/North, clockwise) at radius `r`, as an [x,y] pair.
  const pt = (deg: number, r: number): [number, number] => {
    const a = (deg * Math.PI) / 180;
    return [c + r * Math.sin(a), c - r * Math.cos(a)];
  };
  const xy = ([x, y]: [number, number]) => `${x.toFixed(2)},${y.toFixed(2)}`;

  const rays: React.ReactNode[] = [];
  for (let i = 0; i < 8; i++) {
    const deg = i * 45;
    const cardinal = i % 2 === 0;
    const r = cardinal ? size * 0.44 : size * 0.24;
    const tip = pt(deg, r);
    const left = pt(deg - 22.5, hub);
    const right = pt(deg + 22.5, hub);
    const north = deg === 0;
    // Two-tone point: left half inked, right half cream (classic compass shading); North is fully inked.
    rays.push(
      <polygon key={`${i}L`} points={`${xy(tip)} ${xy(left)} ${xy([c, c])}`} fill={INK} stroke={INK} strokeWidth={0.6} strokeLinejoin="round" />,
    );
    rays.push(
      <polygon
        key={`${i}R`}
        points={`${xy(tip)} ${xy(right)} ${xy([c, c])}`}
        fill={north ? INK : CREAM}
        stroke={INK}
        strokeWidth={0.6}
        strokeLinejoin="round"
      />,
    );
  }

  const lr = size * 0.47; // label radius
  const label = (deg: number, text: string) => {
    const [x, y] = pt(deg, lr);
    return (
      <text
        x={x}
        y={y}
        fill={INK}
        fontSize={size * 0.16}
        fontWeight={700}
        textAnchor="middle"
        dominantBaseline="central"
      >
        {text}
      </text>
    );
  };

  const pad = size * 0.12; // room for the N/E/S/W labels beyond the star tips
  return (
    <svg
      width={size}
      height={size}
      viewBox={`${-pad} ${-pad} ${size + 2 * pad} ${size + 2 * pad}`}
      aria-label="Compass rose: north is up"
      role="img"
      style={{ display: "block" }}
    >
      <circle cx={c} cy={c} r={size * 0.40} fill="none" stroke={INK} strokeWidth={1} opacity={0.5} />
      {rays}
      <circle cx={c} cy={c} r={hub} fill={CREAM} stroke={INK} strokeWidth={0.8} />
      {label(0, "N")}
      {label(90, "E")}
      {label(180, "S")}
      {label(270, "W")}
    </svg>
  );
}
