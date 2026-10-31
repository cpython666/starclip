export const telemetryDesktopAvailable = typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;
export const usageEventNames = ['app_started', 'studio_opened', 'card_exported', 'backup_exported', 'backup_imported', 'update_check_completed', 'font_downloaded', 'update_download_completed', 'update_install_started', 'app_active'] as const;
export type UsageEvent = typeof usageEventNames[number];
export type TelemetryState = {
  revision: number; enabled: boolean; configured: boolean; endpointHost: string | null;
  queuedEvents: Array<{ name: UsageEvent; count: number }>;
  lastResult: 'never' | 'sent' | 'failed' | 'disabled' | 'unconfigured';
  // First characters of the random install ID; null while statistics are off.
  installHint: string | null;
};
export function initialTelemetryState(): TelemetryState {
  return { revision: 0, enabled: false, configured: false, endpointHost: null, queuedEvents: [], lastResult: 'never', installHint: null };
}
async function invoke<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  if (!telemetryDesktopAvailable) throw new Error('使用统计设置仅在桌面版可用。');
  return (await import('@tauri-apps/api/core')).invoke<T>(command, args);
}
export const getTelemetryState = () => invoke<TelemetryState>('get_telemetry_state');
export const setTelemetryEnabled = (enabled: boolean) => invoke<TelemetryState>('set_telemetry_enabled', { enabled });
export const flushTelemetry = () => invoke<TelemetryState>('flush_telemetry');
export const clearTelemetryQueue = () => invoke<TelemetryState>('clear_telemetry_queue');
export const resetTelemetryInstall = () => invoke<TelemetryState>('reset_telemetry_install');
export async function onTelemetryStateChanged(receive: (value: TelemetryState) => void): Promise<() => void> {
  if (!telemetryDesktopAvailable) return () => {};
  return (await import('@tauri-apps/api/event')).listen<TelemetryState>('starclip-telemetry-state', event => receive(event.payload));
}
// The backend owns the persisted preference and batching. Feature actions never wait for analytics.
export function recordStudioOpened(): void {
  if (telemetryDesktopAvailable) void invoke('record_usage_event', { name: 'studio_opened' }).catch(() => {});
}
