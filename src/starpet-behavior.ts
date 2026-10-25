import { cubicBezierEase, sampleHop } from './starpet-motion.ts';

export type PetContext = 'sleep' | 'quiet' | 'focus' | 'social' | 'playful';
export type PetBehavior = 'rest' | 'observe' | 'curious' | 'balance' | 'stretch' | 'nod' | 'hop' | 'nuzzle' | 'tickle' | 'stir';
export interface PetPointer { x: number; y: number; inside: boolean }
export interface PetPose {
  behavior: PetBehavior;
  x: number; y: number; tilt: number; sx: number; sy: number;
  gazeX: number; gazeY: number; squint: number; widen: number; wink: number;
  smile: number; wake: number; energy: number;
}

const durations: Record<PetBehavior, number> = { rest: 0, observe: 3.3, curious: 2.5, balance: 2.8, stretch: 3.2, nod: 1.7, hop: 1.6, nuzzle: 2.4, tickle: 1.8, stir: 2.8 };
const choices: Record<PetContext, PetBehavior[]> = {
  sleep: [], quiet: ['observe', 'nod'], focus: ['observe', 'nod'],
  social: ['observe', 'curious', 'balance', 'stretch'],
  playful: ['observe', 'curious', 'balance', 'stretch', 'hop'],
};
const pulse = (time: number, start: number, rise: number, hold: number, fall: number) =>
  cubicBezierEase((time - start) / rise) * (1 - cubicBezierEase((time - start - rise - hold) / fall));

// Decisions happen between actions, never as per-frame noise. Springs in the SVG keep
// interrupted actions continuous; this small director only supplies attention and intent.
export function createPetBehavior(random: () => number = Math.random) {
  let behavior: PetBehavior = 'rest', previous: PetBehavior = 'rest', context: PetContext = 'social';
  let age = 0, clock = 0, due = 1.4, lastTouch = -Infinity, lastNotice = -Infinity;
  let direction = 1, touchY = 0, watching = false, attentionAge = Infinity;
  let attention = { x: 0, y: 0 };
  const start = (next: PetBehavior, side = direction) => { behavior = next; age = 0; direction = side; };
  const pause = () => {
    previous = behavior; behavior = 'rest'; age = 0;
    due = context === 'focus' || context === 'quiet' ? 4 + random() * 4 : 1.8 + random() * 3.6;
  };
  return {
    touch(nextContext: PetContext, pointer: PetPointer) {
      context = nextContext;
      const repeated = clock - lastTouch < .85;
      lastTouch = clock;
      touchY = pointer.inside ? pointer.y : -1;
      const side = pointer.inside && Math.abs(pointer.x) > .5 ? Math.sign(pointer.x) : direction;
      const next = context === 'sleep' ? 'stir' : context === 'focus' ? 'nod' : repeated ? 'tickle' : 'nuzzle';
      // Repeated presses must not keep restarting anticipation before it can respond.
      if (behavior !== next) start(next, side);
    },
    step(seconds: number, nextContext: PetContext, pointer: PetPointer): PetPose {
      const dt = Number.isFinite(seconds) ? Math.max(0, Math.min(.1, seconds)) : 0;
      clock += dt; age += dt; attentionAge += dt;
      if (nextContext !== context) { context = nextContext; pause(); lastTouch = -Infinity; }
      if (pointer.inside) { attention = { x: pointer.x, y: pointer.y }; attentionAge = 0; }
      if (pointer.inside && !watching && context !== 'sleep' && clock - lastNotice > 3 && clock - lastTouch > 2.4) {
        start(context === 'focus' || context === 'quiet' ? 'nod' : 'curious', pointer.x < 0 ? -1 : 1);
        lastNotice = clock;
      }
      watching = pointer.inside;
      if (behavior !== 'rest' && age >= durations[behavior]) pause();
      if (behavior === 'rest' && age >= due && choices[context].length && !pointer.inside) {
        const available = choices[context].filter(item => item !== previous);
        start(available[Math.min(available.length - 1, Math.floor(random() * available.length))]!, random() < .5 ? -1 : 1);
      }
      const pose: PetPose = { behavior, x: 0, y: 0, tilt: 0, sx: 1, sy: 1, gazeX: 0, gazeY: 0, squint: 0, widen: 0, wink: 0, smile: 0, wake: 0, energy: 0 };
      const a = age, d = direction;
      if (behavior === 'observe') {
        const first = pulse(a, .04, .14, .65, .3), second = pulse(a, 1.28, .16, .5, .55);
        pose.gazeX = d * (5.5 * first - 3.5 * second); pose.gazeY = -2 * first + second;
        pose.tilt = d * (5 * pulse(a, .22, .35, .6, .55) - 3 * pulse(a, 1.48, .3, .45, .65));
        pose.widen = .13 * first;
      } else if (behavior === 'curious') {
        const lean = pulse(a, .17, .42, .7, .85);
        pose.gazeX = d * 5 * pulse(a, 0, .14, 1, .8); pose.gazeY = -1.5;
        pose.x = d * 7 * lean; pose.y = -3 * lean; pose.tilt = d * 10 * lean;
        pose.sx -= .025 * lean; pose.sy += .035 * lean; pose.widen = .24 * lean;
        pose.wink = .6 * pulse(a, 1.35, .14, .12, .28);
      } else if (behavior === 'balance') {
        const sway = pulse(a, .1, .4, .9, .8) * Math.sin((a - .1) * 2.7);
        pose.x = d * 6 * sway; pose.tilt = d * 12 * sway; pose.gazeX = -d * 3 * sway;
        pose.squint = .28 * Math.abs(sway); pose.smile = .45 * Math.abs(sway);
      } else if (behavior === 'stretch') {
        const ready = pulse(a, 0, .2, .05, .3), reach = pulse(a, .35, .6, .65, 1.2);
        pose.sx += .045 * ready - .075 * reach; pose.sy += -.06 * ready + .13 * reach;
        pose.y = 3 * ready - 5 * reach; pose.tilt = d * 4 * reach;
        pose.squint = .93 * reach; pose.smile = .7 * reach; pose.gazeY = -3 * reach;
      } else if (behavior === 'nod') {
        const dip = pulse(a, .22, .23, .12, .5);
        pose.gazeY = -2 * pulse(a, 0, .12, .22, .5); pose.y = 4 * dip;
        pose.sy -= .035 * dip; pose.tilt = d * 3 * dip; pose.squint = .4 * dip; pose.smile = .3 * dip;
      } else if (behavior === 'hop') {
        Object.assign(pose, sampleHop(a - .15)); pose.energy = pulse(a, .3, .12, .08, .7);
        pose.squint = .55 * pulse(a, .1, .15, .05, .25); pose.smile = .8;
      } else if (behavior === 'nuzzle') {
        const soften = pulse(a, .02, .18, .75, 1.25), lean = pulse(a, .12, .35, .6, 1.15);
        pose.x = d * 5 * lean; pose.tilt = d * 9 * lean; pose.y = (touchY < 0 ? -2 : 2) * lean;
        pose.sx += .04 * lean; pose.sy -= .045 * lean; pose.squint = .9 * soften;
        pose.smile = soften; pose.gazeX = d * 3 * lean;
      } else if (behavior === 'tickle') {
        const laugh = pulse(a, 0, .15, .5, 1);
        pose.tilt = d * Math.sin(a * 12) * 9 * laugh; pose.x = Math.sin(a * 12 - .35) * 3 * laugh;
        pose.sx += .045 * laugh; pose.sy -= .05 * laugh; pose.squint = .96 * laugh; pose.smile = laugh;
      } else if (behavior === 'stir') {
        const wake = pulse(a, .25, .5, .35, 1.3);
        pose.tilt = -d * 4 * wake; pose.wake = wake; pose.gazeX = d * 2 * wake;
        pose.squint = .65 * wake; pose.wink = .65 * wake; pose.smile = .25 * wake;
      }
      // Keep looking for a moment after the pointer leaves, then let attention go.
      const attentionWeight = context === 'sleep' || !Number.isFinite(attentionAge) ? 0 : 1 - cubicBezierEase((attentionAge - 1) / 1.3);
      pose.gazeX = pose.gazeX * (1 - attentionWeight) + attention.x * attentionWeight;
      pose.gazeY = pose.gazeY * (1 - attentionWeight) + attention.y * attentionWeight;
      return pose;
    },
  };
}
