export interface Clip {
  id: number;
  content: string;
  title: string;
  kind: 'text' | 'link' | 'code' | 'color';
  language: string | null;
  pinned: boolean;
  groupId: number | null;
  createdAt: number;
  updatedAt: number;
  copyCount: number;
}
export interface Group { id: number; name: string; color: string }
export interface Settings {
  paused: boolean;
  theme: 'dark' | 'light' | 'forest' | 'rose' | 'system';
  showStartup: boolean;
  layout: 'grid' | 'list' | 'horizontal';
  maxHistory: number;
  retentionDays: number;
  alwaysOnTop: boolean;
  shortcut: string;
}
export interface AppState {
  clips: Clip[];
  groups: Group[];
  settings: Settings;
  clipboardError: string | null;
  shortcutError: string | null;
  windowError: string | null;
}
export interface HandwritingAssets {
  available: boolean;
  path: string | null;
  fonts: Array<{ name: string; path: string }>;
  backgrounds: Array<{ name: string; path: string }>;
  error: string | null;
}
export interface HandwritingOptions {
  text: string;
  font: string;
  background: string;
  fontSize: number;
  ink: string;
  padding: number;
}

export type DataDirectoryMode = 'move' | 'use';
export interface DataLocation {
  currentPath: string;
  defaultPath: string;
  configPath: string;
  pendingPath: string | null;
  totalBytes: number;
  databaseBytes: number;
  warnings: string[];
}
export interface DataDirectoryPlan {
  mode: DataDirectoryMode;
  currentPath: string;
  targetPath: string;
  totalBytes: number;
  warnings: string[];
}
