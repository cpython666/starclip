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

// A plump, round-tipped five-point star. Each lobe is a cubic cap between soft
// valleys; the top lobe leans a touch left and the lower "feet" are shorter so
// the face sits in a big, round belly. Delayed tip/side motion stays small.
const lobes = [[-92, 92, 22], [-17, 90, 21], [56, 84, 23], [124, 84, 23], [197, 90, 21]] as const;
const valleyRadius = 62, valleyPull = 22, center = [120, 127] as const, radian = Math.PI / 180;
const tangent = (angle: number) => [-Math.sin(angle * radian), Math.cos(angle * radian)] as const;
export function starOutline(tip: number, side: number) {
  const t = Number.isFinite(tip) ? Math.max(-6, Math.min(6, tip)) : 0;
  const s = Number.isFinite(side) ? Math.max(-5, Math.min(5, side)) : 0;
  const n = (value: number) => Math.round(value * 100) / 100;
  // Top lobe bobs with tip; the side "arms" and feet counter-swing with side.
  const lift = [0, s, -s * .6, s * .6, -s];
  const points = lobes.map(([angle, radius], index) => {
    const reach = radius - (index === 0 ? t : 0);
    return [center[0] + Math.cos(angle * radian) * reach, center[1] + Math.sin(angle * radian) * reach + lift[index]!] as const;
  });
  const valleys = lobes.map(([angle], index) => {
    const middle = (angle + lobes[(index + 1) % 5]![0] + (index === 4 ? 360 : 0)) / 2;
    return { angle: middle, x: center[0] + Math.cos(middle * radian) * valleyRadius, y: center[1] + Math.sin(middle * radian) * valleyRadius };
  });
  let path = `M${n(valleys[4]!.x)} ${n(valleys[4]!.y)}`;
  lobes.forEach(([angle, , cap], index) => {
    const from = valleys[(index + 4) % 5]!, to = valleys[index]!, [x, y] = points[index]!;
    const a = tangent(from.angle), b = tangent(angle), c = tangent(to.angle);
    path += `C${n(from.x + a[0] * valleyPull)} ${n(from.y + a[1] * valleyPull)} ${n(x - b[0] * cap)} ${n(y - b[1] * cap)} ${n(x)} ${n(y)}`;
    path += `C${n(x + b[0] * cap)} ${n(y + b[1] * cap)} ${n(to.x - c[0] * valleyPull)} ${n(to.y - c[1] * valleyPull)} ${n(to.x)} ${n(to.y)}`;
  });
  return `${path}Z`;
}

// The little sprout on the top lobe (星芽 means "star sprout"): where it is rooted.
export function sproutRoot(tip: number) {
  const t = Number.isFinite(tip) ? Math.max(-6, Math.min(6, tip)) : 0;
  const [angle, radius] = lobes[0];
  return { x: center[0] + Math.cos(angle * radian) * (radius - t - 3), y: center[1] + Math.sin(angle * radian) * (radius - t - 3) };
}
