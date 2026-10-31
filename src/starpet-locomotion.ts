import { stepSpring } from './starpet-motion.ts';
import type { Spring } from './starpet-motion.ts';

export type DockEdge = 'left' | 'right' | 'top' | 'bottom';
export type Gait = 'run' | 'climb' | 'fall' | 'cling' | 'lie' | 'hang';
// Window travel in screen space, already normalized to -1..1 per axis.
export interface Travel { x: number; y: number }
export interface LocomotionPose {
  gait: Gait | null; weight: number;
  x: number; y: number; tilt: number; sx: number; sy: number;
  gazeX: number; gazeY: number; widen: number; squint: number; smile: number;
  limbs: Array<[number, number]>; ground: number;
}

const clamp = (value: number, low = -1, high = 1) => Number.isFinite(value) ? Math.max(low, Math.min(high, value)) : 0;

// Turns window speed into a gait. Dominant axis wins: sideways is a run, upward a
// climb, downward a flailing drop. Speed sets both cadence and how far it leans.
export function travelGait(travel: Travel | undefined): { gait: 'run' | 'climb' | 'fall' | null; speed: number; side: number } {
  const x = clamp(travel?.x ?? 0), y = clamp(travel?.y ?? 0);
  const speed = Math.min(1, Math.hypot(x, y));
  if (speed < .06) return { gait: null, speed: 0, side: 1 };
  if (Math.abs(x) >= Math.abs(y) * .8) return { gait: 'run', speed, side: Math.sign(x) || 1 };
  return { gait: y < 0 ? 'climb' : 'fall', speed, side: Math.sign(x) || 1 };
}

// A small stateful animator: the step phase keeps running between frames so the
// stride never restarts, and one weight spring eases between free and moving poses.
export function createLocomotion() {
  let phase = 0, idle = 0, weight: Spring = { value: 0, velocity: 0 };
  let last: Gait | null = null, side = 1;
  return {
    reset() { phase = 0; idle = 0; weight = { value: 0, velocity: 0 }; last = null; side = 1; },
    step(seconds: number, travel: Travel | undefined, dock: DockEdge | null | undefined): LocomotionPose {
      const dt = Number.isFinite(seconds) ? Math.max(0, Math.min(.1, seconds)) : 0;
      const moving = travelGait(travel);
      // Being dragged always wins over resting against an edge.
      const gait: Gait | null = moving.gait ?? (dock === 'left' || dock === 'right' ? 'cling' : dock === 'bottom' ? 'lie' : dock === 'top' ? 'hang' : null);
      if (gait) { last = gait; if (moving.gait) side = moving.side; else if (dock === 'left') side = -1; else if (dock === 'right') side = 1; }
      if (gait !== 'cling' && gait !== 'lie' && gait !== 'hang') idle = 0; else idle += dt;
      weight = stepSpring(weight, gait ? 1 : 0, dt, gait ? 16 : 9, .82);
      const w = Math.max(0, Math.min(1, weight.value)), s = moving.speed, d = side;
      // Cadence rises from a trot (about 2 steps/s) to a dash (about 5 steps/s).
      phase += dt * Math.PI * (moving.gait ? 2 + 3 * s : 1.2);
      const pose: LocomotionPose = { gait: w > .02 ? last : null, weight: w, x: 0, y: 0, tilt: 0, sx: 1, sy: 1, gazeX: 0, gazeY: 0, widen: 0, squint: 0, smile: 0, limbs: [], ground: 1 };
      const stride = Math.sin(phase), bounce = Math.abs(stride), contact = 1 - bounce;
      switch (last) {
        case 'run':
          pose.x = d * 4 * s; pose.y = -7 * bounce * (.5 + s * .5); pose.tilt = d * (6 + 9 * s);
          pose.sx = 1 + .05 * contact * s; pose.sy = 1 - .06 * contact * s + .03 * bounce;
          pose.gazeX = d * 5; pose.gazeY = -.5; pose.widen = .15 + .2 * s; pose.smile = .5;
          // Feet trade places under the belly; the lifted one swings forward.
          pose.limbs = [-1, 1].map(lane => {
            const swing = Math.sin(phase + (lane < 0 ? 0 : Math.PI));
            return [120 + lane * 20 + d * 13 * swing, 207 - 11 * Math.max(0, swing)] as [number, number];
          });
          pose.ground = .6 + .4 * contact;
          break;
        case 'climb':
          pose.y = -4 * bounce; pose.tilt = 5 * Math.sin(phase * .5) * s; pose.sy = 1.05; pose.sx = .97;
          pose.gazeY = -3.5; pose.gazeX = d * 1.5; pose.widen = .2; pose.squint = .2; pose.smile = .3;
          // Alternating reach overhead, like pulling itself up a rope.
          pose.limbs = [[68, 78 - 14 * Math.max(0, stride)], [172, 78 - 14 * Math.max(0, -stride)]];
          pose.ground = .4;
          break;
        case 'fall':
          pose.y = 3 + 2 * Math.sin(phase * 1.5); pose.tilt = 7 * Math.sin(phase * .7); pose.sx = 1.04; pose.sy = .95;
          pose.gazeY = 3; pose.widen = .65; pose.smile = 0;
          // Arms flutter up as if the floor dropped away.
          pose.limbs = [[64 + 4 * stride, 66 - 7 * stride], [176 - 4 * stride, 66 + 7 * stride]];
          pose.ground = .4;
          break;
        case 'cling': {
          // Lean into the wall, hands gripping it; every few seconds peek past the edge.
          const peek = Math.max(0, Math.sin(idle * .9 - 1.4)) ** 4, tap = Math.max(0, Math.sin(idle * 7)) * Math.max(0, Math.sin(idle * .55 + 1));
          pose.x = d * (8 + 6 * peek); pose.tilt = d * (16 + 8 * peek) + 2 * Math.sin(idle * 1.3);
          pose.sx = .96; pose.sy = 1.03 + .01 * Math.sin(idle * 1.9);
          pose.gazeX = -d * 4 * (1 - peek) + d * 5 * peek; pose.gazeY = -.5;
          pose.smile = .35; pose.squint = .25 * peek; pose.widen = .25 * peek;
          pose.limbs = [[120 + d * 90, 98 - 4 * tap], [120 + d * 88, 158 + 2 * Math.sin(idle * 1.9)]];
          pose.ground = 0;
          break;
        }
        case 'lie': {
          // Belly down on the edge, chin on its hands; now and then it lifts its head to look around.
          const look = Math.max(0, Math.sin(idle * .7 - 1.2)) ** 3, rock = Math.sin(idle * 1.4);
          pose.y = 14 - 5 * look; pose.sx = 1.12 - .04 * look; pose.sy = .84 + .05 * look; pose.tilt = 3 * rock * (1 - look);
          pose.gazeX = 4.5 * Math.sin(idle * .9) * look; pose.gazeY = 1.5 - 3 * look; pose.squint = .45 * (1 - look); pose.smile = .55;
          pose.limbs = [[90 - 2 * rock, 205], [150 - 2 * rock, 205]];
          pose.ground = 1.2;
          break;
        }
        case 'hang': {
          // Hanging from the top edge by both hands, swinging like a little pendulum.
          const swing = Math.sin(idle * 1.6), kick = Math.max(0, Math.sin(idle * .5 - 2)) ** 6;
          pose.y = -12; pose.tilt = 7 * swing; pose.sy = 1.06 + .02 * kick; pose.sx = .97 - .02 * kick;
          pose.gazeX = -2.5 * swing; pose.gazeY = 2.5; pose.smile = .45 + .3 * kick; pose.widen = .2 * kick;
          pose.limbs = [[92, 34], [148, 34]];
          pose.ground = 0;
          break;
      }
      }
      return pose;
    },
  };
}
