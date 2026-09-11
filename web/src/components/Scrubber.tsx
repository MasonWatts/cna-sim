import { LEVELS, LEVEL_LABEL, type Level } from "../hierarchy";

/**
 * Timeline control with a granularity selector: play/step at Month, Week, Op-Stage, Phase, or Step
 * level. Prev/Next and Play advance by the chosen level's node boundaries; the slider stays at
 * step resolution for fine scrubbing. Node position shows where you are at the current level.
 */
export function Scrubber({
  step,
  last,
  playing,
  level,
  nodeIndex,
  nodeTotal,
  nodeLabel,
  onLevel,
  onPrev,
  onNext,
  onStep,
  onRestart,
  onTogglePlay,
}: {
  step: number;
  last: number;
  playing: boolean;
  level: Level;
  nodeIndex: number;
  nodeTotal: number;
  nodeLabel: string;
  onLevel: (l: Level) => void;
  onPrev: () => void;
  onNext: () => void;
  onStep: (s: number) => void;
  onRestart: () => void;
  onTogglePlay: () => void;
}) {
  const levelIdx = LEVELS.indexOf(level);
  return (
    <div className="scrubber">
      <span className="level-select" title="Playback granularity — how far Prev/Next/Play jump">
        <button onClick={() => onLevel(LEVELS[Math.max(0, levelIdx - 1)])} title="Zoom out (coarser)">⤢−</button>
        {LEVELS.map((l) => (
          <button key={l} className={l === level ? "active" : ""} onClick={() => onLevel(l)}>
            {LEVEL_LABEL[l]}
          </button>
        ))}
        <button onClick={() => onLevel(LEVELS[Math.min(LEVELS.length - 1, levelIdx + 1)])} title="Zoom in (finer)">⤢+</button>
      </span>

      <span className="transport">
        <button onClick={onRestart} title="Restart">⏮</button>
        <button onClick={onPrev} title={`Previous ${LEVEL_LABEL[level]}`}>◀</button>
        <button onClick={onTogglePlay} title="Play/Pause" style={{ minWidth: 44 }}>
          {playing ? "⏸ Pause" : "▶ Play"}
        </button>
        <button onClick={onNext} title={`Next ${LEVEL_LABEL[level]}`}>▶</button>
      </span>

      <span className="meta node-pos" title={`${LEVEL_LABEL[level]} position`}>
        {level === "step" ? `step ${step}` : `${nodeLabel}  (${nodeIndex + 1}/${nodeTotal})`}
      </span>

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
