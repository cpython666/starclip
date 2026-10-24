export interface Spring { value: number; velocity: number }

// Exact damped spring step: the same elapsed time gives the same response at 30/60/144 Hz.
export function stepSpring(state: Spring, target: number, seconds: number, speed = 15, damping = .7): Spring {
  if (!Number.isFinite(target)) return { value: 0, velocity: 0 };
  if (![state.value, state.velocity, seconds].every(Number.isFinite)) return { value: target, velocity: 0 };
  if (seconds <= 0) return state;
  const decay = speed * damping, frequency = speed * Math.sqrt(1 - damping * damping);
  const distance = state.value - target, factor = Math.exp(-decay * seconds);
  const sin = Math.sin(frequency * seconds), cos = Math.cos(frequency * seconds);
  return {
    value: target + factor * (distance * cos + (state.velocity + decay * distance) / frequency * sin),
    velocity: factor * (state.velocity * cos - (decay * state.velocity + speed * speed * distance) / frequency * sin),
  };
}

const hopFrames = [
  [0, 0, 1, 1], [.14, 5, 1.14, .82], [.27, -10, .92, 1.13],
  [.42, -22, .98, 1.035], [.59, 0, .95, 1.08], [.67, 5, 1.18, .79],
  [.82, -4, .97, 1.045], [1.08, 0, 1, 1],
] as const;

export function sampleHop(seconds: number) {
  if (!Number.isFinite(seconds) || seconds <= 0 || seconds >= 1.08) return { y: 0, sx: 1, sy: 1 };
  const index = hopFrames.findIndex(frame => frame[0] >= seconds);
  const before = hopFrames[index - 1]!, after = hopFrames[index]!;
  const progress = (seconds - before[0]) / (after[0] - before[0]);
  const ease = (1 - Math.cos(progress * Math.PI)) / 2;
  return {
    y: before[1] + (after[1] - before[1]) * ease,
    sx: before[2] + (after[2] - before[2]) * ease,
    sy: before[3] + (after[3] - before[3]) * ease,
  };
}

export interface HopState { y: Spring; sx: Spring; sy: Spring }
export function stepHop(state: HopState, age: number, seconds: number): HopState {
  const target = sampleHop(age);
  return {
    y: stepSpring(state.y, target.y, seconds, 42, .72),
    sx: stepSpring(state.sx, target.sx, seconds, 42, .72),
    sy: stepSpring(state.sy, target.sy, seconds, 42, .72),
  };
}

// Unequal intervals and an occasional double blink avoid a metronomic loop.
const blinkIntervals = [2.7, 4.6, 3.2, 5.5, 3.8, 6.1];
const blinkPeriod = blinkIntervals.reduce((sum, interval) => sum + interval, 0);
export function blinkOpenness(seconds: number): number {
  if (!Number.isFinite(seconds) || seconds < 0) return 1;
  let phase = seconds % blinkPeriod;
  for (let index = 0; index < blinkIntervals.length; index++) {
    if (phase < blinkIntervals[index]!) {
      const closing = phase > .28 && index % 3 === 2 ? phase - .28 : phase;
      if (closing > .2) return 1;
      return 1 - .94 * Math.sin(Math.PI * closing / .2) ** 2;
    }
    phase -= blinkIntervals[index]!;
  }
  return 1;
}

// Only the five tips flex; the face remains inside a stable, recognizable star.
export function starOutline(tip: number, side: number) {
  const t = Number.isFinite(tip) ? Math.max(-6, Math.min(6, tip)) : 0;
  const s = Number.isFinite(side) ? Math.max(-5, Math.min(5, side)) : 0;
  const n = (value: number) => Math.round(value * 100) / 100;
  return `M112 ${n(42 + t)}Q120 ${n(25 + t)} 128 ${n(42 + t)}L148 80Q151 86 158 87L197 ${n(91 + s)}Q216 ${n(93 + s)} 202 ${n(108 + s)}L173 137Q168 142 170 150L178 ${n(189 - s)}Q182 ${n(209 - s)} 164 199L128 180Q121 176 114 180L78 199Q60 ${n(209 + s)} 64 ${n(189 + s)}L72 150Q74 142 68 137L39 ${n(108 - s)}Q25 ${n(93 - s)} 44 ${n(91 - s)}L83 87Q90 86 93 80Z`;
}
