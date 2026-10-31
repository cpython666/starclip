import { invoke } from '@tauri-apps/api/core';
import type { StarPetMoodId } from './starpet';
import type { DockEdge } from './starpet-locomotion';

export const defaultStarPetColor = '#FFD66E';
export interface StarPetPreferences { enabled: boolean; mood: StarPetMoodId; color: string; size: number }
export type StarPetPreferencePatch = Partial<StarPetPreferences>;
export const starPetDesktopAvailable = '__TAURI_INTERNALS__' in window;
let demo: StarPetPreferences = { enabled: false, mood: 'idle', color: defaultStarPetColor, size: 100 };
const listeners = new Set<(preferences: StarPetPreferences) => void>();

export async function getStarPetPreferences(): Promise<StarPetPreferences> {
  if (!starPetDesktopAvailable) return { ...demo };
  return (await import('@tauri-apps/api/core')).invoke('get_starpet_preferences');
}
export async function setStarPetPreferences(preferences: StarPetPreferencePatch): Promise<StarPetPreferences> {
  const patch = Object.fromEntries(Object.entries(preferences).filter(([, value]) => value !== undefined)) as StarPetPreferencePatch;
  if (patch.size !== undefined && (!Number.isInteger(patch.size) || patch.size < 70 || patch.size > 160)) {
    throw new Error('星伴大小需在 70% 到 160% 之间。');
  }
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
export async function onStarPetWindowBlur(callback: () => void): Promise<() => void> {
  if (!starPetDesktopAvailable) return () => {};
  return (await import('@tauri-apps/api/event')).listen('tauri://blur', callback, {
    target: { kind: 'Window', label: 'starpet' },
  });
}
export async function showStarPetMainWindow(): Promise<void> {
  if (starPetDesktopAvailable) await (await import('@tauri-apps/api/core')).invoke('show_main_window');
}
export async function dragStarPet(): Promise<void> {
  if (starPetDesktopAvailable) await invoke('starpet_start_dragging');
}
// Snaps the pet window onto a nearby work-area edge; null means it stays where it was dropped.
export async function dockStarPet(): Promise<DockEdge | null> {
  if (!starPetDesktopAvailable) return null;
  const edge = await invoke<string | null>('starpet_dock');
  return edge === 'left' || edge === 'right' || edge === 'top' || edge === 'bottom' ? edge : null;
}
// Native dragging moves the window outside the webview, so window moves are the only drag signal.
export async function onStarPetMoved(callback: (position: { x: number; y: number }) => void): Promise<() => void> {
  if (!starPetDesktopAvailable) return () => {};
  return (await import('@tauri-apps/api/event')).listen<{ x: number; y: number }>('tauri://move', event => callback(event.payload), {
    target: { kind: 'Window', label: 'starpet' },
  });
}
