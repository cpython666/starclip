import { changeDemoGrowth, emptyGrowth, growthArchive, localDay, parseGrowthArchive } from './starpet-growth';
import type { ActionId, GrowthChange, GrowthState, Slot } from './starpet-growth';

const desktop = typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;
let demo = emptyGrowth();
const listeners = new Set<(state: GrowthState) => void>();
const clone = () => structuredClone(demo);
async function invoke<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  return (await import('@tauri-apps/api/core')).invoke<T>(command, args);
}
function publish(value: GrowthState): GrowthState {
  demo = value;
  for (const listener of listeners) listener(clone());
  return clone();
}
async function change(command: string, args: Record<string, unknown>, update: GrowthChange): Promise<GrowthState> {
  if (desktop) return invoke(command, args);
  return publish(changeDemoGrowth(demo, update));
}
export async function getStarPetGrowth(): Promise<GrowthState> {
  return desktop ? invoke('get_starpet_growth') : clone();
}
export function setStarPetGrowthEnabled(enabled: boolean): Promise<GrowthState> {
  return change('set_starpet_growth_enabled', { enabled }, { kind: 'enabled', enabled });
}
export function interactStarPet(action: ActionId): Promise<GrowthState> {
  return change('interact_starpet', { action }, { kind: 'action', action });
}
export function equipStarPetItem(slot: Slot, itemId: string | null): Promise<GrowthState> {
  return change('equip_starpet_item', { slot, itemId }, { kind: 'equip', slot, itemId });
}
export async function onStarPetGrowthChanged(callback: (state: GrowthState) => void): Promise<() => void> {
  if (desktop) return (await import('@tauri-apps/api/event')).listen<GrowthState>('starpet-growth-changed', event => callback(event.payload));
  listeners.add(callback);
  return () => { listeners.delete(callback); };
}
export async function exportStarPetGrowth(): Promise<string | null> {
  if (desktop) return invoke('export_starpet_growth');
  const filename = `starclip-starpet-${localDay()}.json`;
  const url = URL.createObjectURL(new Blob([growthArchive(demo)], { type: 'application/json' }));
  const link = document.createElement('a');
  link.href = url; link.download = filename;
  document.body.appendChild(link); link.click(); link.remove();
  setTimeout(() => URL.revokeObjectURL(url), 1000);
  return filename;
}
function replacementRevision(expectedRevision: number): number {
  if (expectedRevision !== demo.revision) throw new Error('成长进度已变化，请重新查看后再确认。');
  if (demo.revision >= Number.MAX_SAFE_INTEGER) throw new Error('成长存档修订号已达上限。');
  return demo.revision + 1;
}
export async function importStarPetGrowth(data: string, expectedRevision: number): Promise<GrowthState> {
  const value = parseGrowthArchive(data);
  if (desktop) return invoke('import_starpet_growth', { data, expectedRevision });
  return publish({ ...value, revision: replacementRevision(expectedRevision) });
}
export async function resetStarPetGrowth(expectedRevision: number): Promise<GrowthState> {
  if (desktop) return invoke('reset_starpet_growth', { expectedRevision });
  return publish({ ...emptyGrowth(), revision: replacementRevision(expectedRevision) });
}
