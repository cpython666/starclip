import { invoke } from '@tauri-apps/api/core';
import type { StarPetMoodId } from './starpet';

export const defaultStarPetColor = '#FFD66E';
export interface StarPetPreferences { enabled: boolean; mood: StarPetMoodId; color: string }
export type StarPetPreferencePatch = Partial<StarPetPreferences>;
export const starPetDesktopAvailable = '__TAURI_INTERNALS__' in window;
let demo: StarPetPreferences = { enabled: false, mood: 'idle', color: defaultStarPetColor };
const listeners = new Set<(preferences: StarPetPreferences) => void>();

export async function getStarPetPreferences(): Promise<StarPetPreferences> {
  if (!starPetDesktopAvailable) return { ...demo };
  return (await import('@tauri-apps/api/core')).invoke('get_starpet_preferences');
}
export async function setStarPetPreferences(preferences: StarPetPreferencePatch): Promise<StarPetPreferences> {
  const patch = Object.fromEntries(Object.entries(preferences).filter(([, value]) => value !== undefined)) as StarPetPreferencePatch;
  if (patch.color !== undefined) {
    if (!/^#[0-9a-f]{6}$/i.test(patch.color)) throw new Error('星伴颜色需为六位 HEX，例如 #FFD66E。');
    patch.color = patch.color.toUpperCase();
  }
  if (!starPetDesktopAvailable) {
    demo = { ...demo, ...patch, enabled: false };
    listeners.forEach(listener => listener({ ...demo }));
    return { ...demo };
  }
  return (await import('@tauri-apps/api/core')).invoke('set_starpet_preferences', { preferences: patch });
}
export async function onStarPetChanged(callback: (preferences: StarPetPreferences) => void): Promise<() => void> {
  if (starPetDesktopAvailable) return (await import('@tauri-apps/api/event')).listen<StarPetPreferences>('starpet-changed', event => callback(event.payload));
  listeners.add(callback);
  return () => { listeners.delete(callback); };
}
export async function showStarPetMainWindow(): Promise<void> {
  if (starPetDesktopAvailable) await (await import('@tauri-apps/api/core')).invoke('show_main_window');
}
export async function dragStarPet(): Promise<void> {
  if (starPetDesktopAvailable) await invoke('starpet_start_dragging');
}
