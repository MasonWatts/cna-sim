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
  onSlider,
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
  onSlider: (v: number) => void;
  onRestart: () => void;
  onTogglePlay: () => void;
}) {
  // The slider and counter track the selected granularity: over steps at Step level, over nodes
  // (weeks / op-stages / phases) otherwise.
  const isStep = level === "step";
  const sliderValue = isStep ? step : nodeIndex;
  const sliderMax = isStep ? last : Math.max(0, nodeTotal - 1);
  const counter = isStep ? `step ${step} / ${last}` : `${LEVEL_LABEL[level]} ${nodeIndex + 1} / ${nodeTotal}`;
  return (
    <div className="scrubber">
      <span className="level-select" title="Playback granularity — how far Prev/Next/Play jump">
        {LEVELS.map((l) => (
          <button key={l} className={l === level ? "active" : ""} onClick={() => onLevel(l)}>
            {LEVEL_LABEL[l]}
          </button>
        ))}
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
        {isStep ? `step ${step}` : nodeLabel}
      </span>

      <span className="slider-wrap">
        <input
          type="range"
          min={0}
          max={sliderMax}
          value={sliderValue}
          onChange={(e) => onSlider(Number(e.target.value))}
        />
        {!isStep && sliderMax > 0 && sliderMax <= 200 && (
          <span className="ticks" aria-hidden>
            {Array.from({ length: sliderMax + 1 }, (_, i) => (
              <span key={i} className={i === sliderValue ? "tick on" : "tick"} />
            ))}
          </span>
        )}
      </span>
      <span className="meta" style={{ minWidth: 110, textAlign: "right" }}>
        {counter}
      </span>
    </div>
  );
}
