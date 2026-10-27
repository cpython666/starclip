import type { ActionId } from './starpet-growth';
import type { StarPetMoodId } from './starpet';

export type PetAgentStatus = 'idle' | 'thinking' | 'working' | 'waiting' | 'success' | 'error';
export interface PetSignal { source: string; status?: PetAgentStatus; action?: ActionId; message?: string; expiresAt: number }
export interface PetSnapshot { revision: number; signal: PetSignal | null }
export const petStatusMood: Record<PetAgentStatus, StarPetMoodId> = { idle: 'idle', thinking: 'thinking', working: 'writing', waiting: 'waiting', success: 'success', error: 'error' };
export const petStatusLabel: Record<PetAgentStatus, string> = { idle: '空闲', thinking: '思考中', working: '工作中', waiting: '等待中', success: '已完成', error: '遇到问题' };
export async function getStarPetSignal(): Promise<PetSnapshot> {
  if (!('__TAURI_INTERNALS__' in window)) return { revision: 0, signal: null };
  return (await import('@tauri-apps/api/core')).invoke('get_starpet_signal');
}
export async function onStarPetSignal(callback: (snapshot: PetSnapshot) => void): Promise<() => void> {
  if (!('__TAURI_INTERNALS__' in window)) return () => {};
  return (await import('@tauri-apps/api/event')).listen<PetSnapshot>('starpet-signal', event => callback(event.payload));
}
