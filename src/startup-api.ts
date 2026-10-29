export type StartupPreferences = {
  autoLaunch: boolean;
  startHidden: boolean;
  showTrayIcon: boolean;
};
export type StartupPreferencesPatch = Partial<StartupPreferences>;
export const startupDesktopAvailable = typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;

export function initialStartupPreferences(): StartupPreferences {
  return { autoLaunch: false, startHidden: false, showTrayIcon: true };
}

async function invoke<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  if (!startupDesktopAvailable) throw new Error('启动设置仅在桌面版可用。');
  return (await import('@tauri-apps/api/core')).invoke<T>(command, args);
}

export const getStartupPreferences = () => invoke<StartupPreferences>('get_startup_preferences');
export const setStartupPreferences = (patch: StartupPreferencesPatch) => invoke<StartupPreferences>('set_startup_preferences', { patch });
export const quitStarclip = () => invoke<void>('quit_starclip');
