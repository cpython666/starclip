import catalog from './starpet-growth-catalog.json' with { type: 'json' };

export type Slot = 'back' | 'face' | 'hand' | 'charm';
export type TaskId = 'greet' | 'play' | 'create';
export type ActionId = 'greet' | 'highfive' | 'stretch' | 'twirl' | 'heart' | 'celebrate';
export type Equipment = Record<Slot, string | null>;
export interface GrowthState {
  version: 1;
  enabled: boolean;
  xp: number;
  day: string;
  completedTasks: TaskId[];
  interactionDays: number;
  playDays: number;
  lastInteractionDay: string;
  achievements: string[];
  unlockedItems: string[];
  equipment: Equipment;
  revision: number;
}
export const growthCatalog = catalog as {
  maxLevel: number;
  tasks: { id: TaskId; name: string; description: string; xp: number }[];
  actions: { id: ActionId; name: string; level: number; task: TaskId; message: string }[];
  items: { id: string; name: string; slot: Slot; level: number; achievement?: string; description: string }[];
  achievements: { id: string; name: string; description: string; reward: string }[];
};
export const growthSlots: { id: Slot; name: string }[] = [
  { id: 'back', name: '背部' }, { id: 'face', name: '面部' },
  { id: 'hand', name: '手持' }, { id: 'charm', name: '点缀' },
];
export const maxGrowthXp = 10_000_000;
export const maxGrowthArchiveBytes = 64 * 1024;
export function emptyGrowth(): GrowthState {
  return { version: 1, enabled: false, xp: 0, day: '', completedTasks: [], interactionDays: 0, playDays: 0, lastInteractionDay: '', achievements: [], unlockedItems: [], equipment: { back: null, face: null, hand: null, charm: null }, revision: 0 };
}
export function xpForLevel(level: number): number {
  const n = Math.max(0, Math.min(growthCatalog.maxLevel - 1, Math.trunc(level) - 1));
  return 40 * n + 5 * n * (n - 1);
}
export function levelForXp(xp: number): number {
  let level = 1;
  while (level < growthCatalog.maxLevel && xp >= xpForLevel(level + 1)) level++;
  return level;
}
export function levelProgress(xp: number) {
  const level = levelForXp(xp), current = xp - xpForLevel(level);
  const needed = level === growthCatalog.maxLevel ? 0 : xpForLevel(level + 1) - xpForLevel(level);
  return { level, current, needed, percent: needed ? Math.min(100, Math.max(0, current / needed * 100)) : 100 };
}
export function availableActions(state: GrowthState) {
  return growthCatalog.actions.filter(action => action.level <= levelForXp(state.xp));
}
export function localDay(date = new Date()): string {
  return `${date.getFullYear()}-${String(date.getMonth() + 1).padStart(2, '0')}-${String(date.getDate()).padStart(2, '0')}`;
}
function validDay(value: unknown): value is string {
  if (value === '') return true;
  if (typeof value !== 'string' || !/^\d{4}-\d{2}-\d{2}$/.test(value)) return false;
  const parsed = new Date(`${value}T00:00:00Z`);
  return Number.isFinite(parsed.getTime()) && parsed.toISOString().slice(0, 10) === value && value >= '1970-01-01';
}
function object(value: unknown, keys: readonly string[]): value is Record<string, unknown> {
  return value !== null && typeof value === 'object' && !Array.isArray(value)
    && Object.keys(value).length === keys.length && Object.keys(value).every(key => keys.includes(key));
}
function integer(value: unknown, max: number): value is number {
  return typeof value === 'number' && Number.isSafeInteger(value) && value >= 0 && value <= max;
}
function uniqueIds(value: unknown, allowed: string[]): value is string[] {
  return Array.isArray(value) && value.length <= allowed.length && new Set(value).size === value.length
    && value.every(id => typeof id === 'string' && allowed.includes(id));
}
export function validateGrowthState(value: unknown): GrowthState {
  const fail = () => { throw new Error('成长存档格式无效，现有进度未改动。'); };
  if (!object(value, Object.keys(emptyGrowth()))) return fail();
  if (value.version !== 1 || typeof value.enabled !== 'boolean' || !integer(value.xp, maxGrowthXp)
    || !integer(value.revision, Number.MAX_SAFE_INTEGER) || !integer(value.interactionDays, 1_000_000)
    || !integer(value.playDays, 1_000_000) || value.playDays > value.interactionDays
    || !validDay(value.day) || !validDay(value.lastInteractionDay) || value.lastInteractionDay > value.day
    || !uniqueIds(value.completedTasks, growthCatalog.tasks.map(task => task.id))
    || !uniqueIds(value.achievements, growthCatalog.achievements.map(achievement => achievement.id))
    || !uniqueIds(value.unlockedItems, growthCatalog.items.map(item => item.id))
    || !object(value.equipment, growthSlots.map(slot => slot.id))) return fail();
  if ((!value.day && (value.completedTasks.length || value.interactionDays || value.playDays))
    || (!!value.interactionDays !== !!value.lastInteractionDay)) return fail();
  for (const { id: slot } of growthSlots) {
    const id = value.equipment[slot];
    if (id !== null && (typeof id !== 'string' || !value.unlockedItems.includes(id)
      || !growthCatalog.items.some(item => item.id === id && item.slot === slot))) return fail();
  }
  return structuredClone(value) as unknown as GrowthState;
}
export function parseGrowthArchive(data: string): GrowthState {
  if (new TextEncoder().encode(data).length > maxGrowthArchiveBytes) throw new Error('成长存档不能超过 64 KiB。');
  let archive: unknown;
  try { archive = JSON.parse(data); } catch { throw new Error('请选择有效的成长 JSON 存档。'); }
  if (!object(archive, ['format', 'version', 'growth']) || archive.format !== 'starclip-starpet' || archive.version !== 1) {
    throw new Error('这不是受支持的星伴成长存档，剪贴历史备份不能在这里导入。');
  }
  return validateGrowthState(archive.growth);
}
export function growthArchive(state: GrowthState): string {
  return JSON.stringify({ format: 'starclip-starpet', version: 1, growth: validateGrowthState(state) }, null, 2);
}

// Browser-only simulation; the desktop awards and persists progress in Rust.
function unlock(state: GrowthState) {
  const conditions: Record<string, boolean> = {
    'seven-days': state.interactionDays >= 7,
    'play-five': state.playDays >= 5,
    'level-five': levelForXp(state.xp) >= 5,
    'level-ten': levelForXp(state.xp) >= 10,
  };
  for (const achievement of growthCatalog.achievements) {
    if (conditions[achievement.id] && !state.achievements.includes(achievement.id)) state.achievements.push(achievement.id);
  }
  for (const item of growthCatalog.items) {
    const earned = item.achievement ? state.achievements.includes(item.achievement) : levelForXp(state.xp) >= item.level;
    if (earned && !state.unlockedItems.includes(item.id)) state.unlockedItems.push(item.id);
  }
}
export type GrowthChange = { kind: 'enabled'; enabled: boolean } | { kind: 'action'; action: ActionId }
  | { kind: 'equip'; slot: Slot; itemId: string | null };
export function changeDemoGrowth(previous: GrowthState, change: GrowthChange, day = localDay()): GrowthState {
  const state = structuredClone(previous);
  if (!validDay(day) || !day) throw new Error('本机日期无效。');
  if (change.kind === 'enabled') state.enabled = change.enabled;
  else if (change.kind === 'equip') {
    if (!growthSlots.some(slot => slot.id === change.slot)) throw new Error('未知的装扮位置。');
    if (change.itemId !== null && (!state.unlockedItems.includes(change.itemId)
      || !growthCatalog.items.some(item => item.id === change.itemId && item.slot === change.slot))) throw new Error('这件装饰尚未获得或不属于这个位置。');
    state.equipment[change.slot] = change.itemId;
    if (state.enabled) {
      if (change.itemId && !state.achievements.includes('first-outfit')) state.achievements.push('first-outfit');
      unlock(state);
    }
  } else {
    const action = growthCatalog.actions.find(item => item.id === change.action);
    if (!action || action.level > levelForXp(state.xp)) throw new Error('这个动作尚未解锁。');
    if (!state.enabled || day < state.day) return state;
    if (day > state.day) { state.day = day; state.completedTasks = []; }
    if (day > state.lastInteractionDay) {
      state.lastInteractionDay = day;
      state.interactionDays = Math.min(1_000_000, state.interactionDays + 1);
    }
    if (!state.completedTasks.includes(action.task)) {
      state.completedTasks.push(action.task);
      state.xp = Math.min(maxGrowthXp, state.xp + growthCatalog.tasks.find(task => task.id === action.task)!.xp);
      if (action.task === 'play') state.playDays = Math.min(1_000_000, state.playDays + 1);
      if (action.task === 'greet' && !state.achievements.includes('first-meeting')) state.achievements.push('first-meeting');
    }
    unlock(state);
  }
  if (JSON.stringify(state) !== JSON.stringify(previous)) {
    if (previous.revision >= Number.MAX_SAFE_INTEGER) throw new Error('成长存档修订号已达上限。');
    state.revision = previous.revision + 1;
  }
  return state;
}
