/** Timeline control: play/pause, step buttons, and a slider over all recorded events. */
export function Scrubber({
  step,
  last,
  playing,
  onStep,
  onTogglePlay,
}: {
  step: number;
  last: number;
  playing: boolean;
  onStep: (s: number) => void;
  onTogglePlay: () => void;
}) {
  const clamp = (s: number) => Math.max(0, Math.min(last, s));
  return (
    <div className="scrubber">
      <button onClick={() => onStep(0)} title="Restart">⏮</button>
      <button onClick={() => onStep(clamp(step - 1))} title="Previous">◀</button>
      <button onClick={onTogglePlay} title="Play/Pause" style={{ minWidth: 44 }}>
        {playing ? "⏸ Pause" : "▶ Play"}
      </button>
      <button onClick={() => onStep(clamp(step + 1))} title="Next">▶</button>
      <input
        type="range"
        min={0}
        max={last}
        value={step}
        onChange={(e) => onStep(Number(e.target.value))}
      />
      <span className="meta" style={{ minWidth: 90, textAlign: "right" }}>
        step {step} / {last}
      </span>
    </div>
  );
}
