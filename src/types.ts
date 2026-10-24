export interface Clip {
  id: number;
  content: string;
  title: string;
  kind: 'text' | 'link' | 'code' | 'color';
  language: string | null;
  pinned: boolean;
  keepForever: boolean;
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
  stickerPack: 'auto' | 'none' | 'cosmos' | 'cats' | 'puppies' | 'garden';
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

export interface CleanupFilter {
  minBytes: number;
  olderThanDays: number;
  kind: 'all' | Clip['kind'];
  includeGrouped: boolean;
}
export interface CleanupPreview {
  token: string;
  totalCount: number;
  protectedCount: number;
  matchedCount: number;
  contentBytes: number;
  databaseBytes: number;
  reusableBytes: number;
  items: Array<{ id: number; title: string; kind: Clip['kind']; bytes: number; updatedAt: number }>;
  truncated: boolean;
}
export interface CleanupResult {
  deletedCount: number;
  contentBytes: number;
  databaseBytesBefore: number;
  databaseBytesAfter: number;
  releasedBytes: number;
  warning: string | null;
}
