import type { ActionId } from './starpet-growth';

export const actionDuration: Record<ActionId, number> = { greet: 1.8, highfive: 1.4, stretch: 2.2, twirl: 1.8, heart: 2, celebrate: 2.4 };
export const actionMood: Record<ActionId, string> = { greet: 'greet', highfive: 'happy', stretch: 'stretch', twirl: 'happy', heart: 'love', celebrate: 'celebrate' };

export function actionPose(action: ActionId | null, age: number, reduced = false) {
  const pose = { y: 0, rotation: 0, sx: 1, sy: 1, lift: 0, energy: 0 };
  if (!action || reduced) return pose;
  const p = Math.min(1, Math.max(0, Number.isFinite(age) ? age / actionDuration[action] : 0));
  if (p === 0 || p === 1) return pose;
  const swell = Math.sin(Math.PI * p) ** 2;
  switch (action) {
    case 'greet': pose.rotation = Math.sin(p * Math.PI * 4) * 5 * swell; break;
    case 'highfive': pose.y = -4 * swell; pose.lift = 14 * swell; pose.energy = swell; break;
    case 'stretch': pose.sy = 1 + .08 * swell; pose.sx = 1 - .06 * swell; pose.y = -3 * swell; break;
    case 'twirl':
      pose.rotation = 360 * p * p * (3 - 2 * p);
      // Keep the star and its cape inside the viewBox during the turn.
      pose.sx = pose.sy = 1 - .28 * swell;
      pose.y = -3 * swell;
      break;
    case 'heart': pose.rotation = -7 * swell; pose.sx = pose.sy = 1 + .025 * swell; break;
    case 'celebrate': pose.y = -7 * Math.sin(p * Math.PI * 3) ** 2 * swell; pose.energy = swell; break;
  }
  return pose;
}
