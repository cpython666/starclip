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

// Cubic Bézier (0,0), (1/3,0), (2/3,1), (1,1): each phase eases to rest.
export function cubicBezierEase(progress: number): number {
  const t = Number.isFinite(progress) ? Math.max(0, Math.min(1, progress)) : 0;
  return 3 * (1 - t) * t * t + t * t * t;
}

// Time, grounded x/y, scale x/y; flight position follows the spatial curve below.
const hopFrames = [
  [0, 0, 0, 1, 1], [.14, -1, 5, 1.14, .82], [.27, 3, -10, .92, 1.13],
  [.42, 6, -26, .98, 1.035], [.59, 1, 0, .95, 1.08], [.67, 0, 5, 1.18, .79],
  [.82, -1, -4, .97, 1.045], [1.08, 0, 0, 1, 1],
] as const;

export function sampleHop(seconds: number) {
  if (!Number.isFinite(seconds) || seconds <= 0 || seconds >= 1.08) return { x: 0, y: 0, sx: 1, sy: 1 };
  const index = hopFrames.findIndex(frame => frame[0] >= seconds);
  const before = hopFrames[index - 1]!, after = hopFrames[index]!;
  const progress = (seconds - before[0]) / (after[0] - before[0]);
  const ease = cubicBezierEase(progress);
  let x = before[1] + (after[1] - before[1]) * ease;
  let y = before[2] + (after[2] - before[2]) * ease;
  if (seconds >= .14 && seconds <= .59) {
    // Spatial cubic Bézier: (-1,5) → (7,-39) → (7,-39) → (1,0).
    // Its rounded crest is about 29 px high; ease time only at takeoff/contact.
    const t = cubicBezierEase((seconds - .14) / (.59 - .14)), u = 1 - t;
    x = -(u ** 3) + 21 * u * u * t + 21 * u * t * t + t ** 3;
    y = 5 * u ** 3 - 117 * u * u * t - 117 * u * t * t;
  }
  return {
    x, y,
    sx: before[3] + (after[3] - before[3]) * ease,
    sy: before[4] + (after[4] - before[4]) * ease,
  };
}

export interface HopState { x: Spring; y: Spring; sx: Spring; sy: Spring }
export function stepHop(state: HopState, age: number, seconds: number): HopState {
  const target = sampleHop(age);
  return {
    x: stepSpring(state.x, target.x, seconds, 42, .72),
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

// Rounded cubic lobes traced from the generated concept; delayed tip motion stays small.
export function starOutline(tip: number, side: number) {
  const t = Number.isFinite(tip) ? Math.max(-6, Math.min(6, tip)) : 0;
  const s = Number.isFinite(side) ? Math.max(-5, Math.min(5, side)) : 0;
  const n = (value: number) => Math.round(value * 100) / 100;
  return `M86 72C102 ${n(51 + t)} 117 ${n(30 + t)} 132 ${n(35 + t)}C148 ${n(40 + t)} 159 65 169 83C187 ${n(89 + s)} 209 ${n(98 + s)} 211 ${n(114 + s)}C212 ${n(130 + s)} 197 145 181 157C187 ${n(178 - s)} 182 ${n(199 - s)} 168 ${n(202 - s)}C153 ${n(205 - s)} 132 197 117 190C98 198 74 ${n(202 + s)} 59 ${n(196 + s)}C43 ${n(188 + s)} 46 165 51 150C37 ${n(134 - s)} 25 ${n(113 - s)} 33 ${n(98 - s)}C42 ${n(81 - s)} 64 75 86 72Z`;
}
