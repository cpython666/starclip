import type { StarPetMoodId } from './starpet';

export interface StarPetPreferences { enabled: boolean; mood: StarPetMoodId }
export const starPetDesktopAvailable = '__TAURI_INTERNALS__' in window;
let demo: StarPetPreferences = { enabled: false, mood: 'idle' };
const listeners = new Set<(preferences: StarPetPreferences) => void>();

export async function getStarPetPreferences(): Promise<StarPetPreferences> {
  if (!starPetDesktopAvailable) return { ...demo };
  return (await import('@tauri-apps/api/core')).invoke('get_starpet_preferences');
}
export async function setStarPetPreferences(preferences: StarPetPreferences): Promise<StarPetPreferences> {
  if (!starPetDesktopAvailable) {
    demo = { ...preferences, enabled: false };
    listeners.forEach(listener => listener({ ...demo }));
    return { ...demo };
  }
  return (await import('@tauri-apps/api/core')).invoke('set_starpet_preferences', { preferences });
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
  if (starPetDesktopAvailable) await (await import('@tauri-apps/api/core')).invoke('starpet_start_dragging');
}
