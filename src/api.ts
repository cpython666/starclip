import type { AppState, Clip, CleanupFilter, CleanupPreview, CleanupResult, DataDirectoryMode, DataDirectoryPlan, DataLocation, Group, HandwritingAssets, HandwritingOptions, Settings } from './types';
import { download, languages } from './snippets';

export const isDesktop = '__TAURI_INTERNALS__' in window;
const now = Date.now();
const samples: Array<Pick<Clip, 'title' | 'content' | 'kind' | 'language' | 'groupId' | 'pinned'>> = [
  { title: '让灵感有迹可循', content: '有些灵感像流星，只出现一瞬。\n\n把值得留下的文字、链接与代码，收进自己的小小星系。', kind: 'text', language: null, groupId: 1, pinned: true },
  { title: '一个温柔的问候', content: 'fn main() {\n    let universe = "你好，星笺";\n    println!("{universe} ✦");\n}', kind: 'code', language: 'rust', groupId: 2, pinned: true },
  { title: 'Tauri · 小巧的桌面应用', content: 'https://v2.tauri.app/', kind: 'link', language: null, groupId: 2, pinned: false },
  { title: '下一次灵感，留一点余地', content: '设计不是把每个角落填满，\n而是让重要的东西拥有呼吸的空间。', kind: 'text', language: null, groupId: 1, pinned: false },
  { title: '星尘金', content: '#E9C98B', kind: 'color', language: null, groupId: 3, pinned: false },
  { title: '一个轻巧的 debounce', content: 'function debounce(fn, delay = 300) {\n  let timer;\n  return (...args) => {\n    clearTimeout(timer);\n    timer = setTimeout(() => fn(...args), delay);\n  };\n}', kind: 'code', language: 'javascript', groupId: 2, pinned: true },
  { title: '今晚的待办', content: '☑ 留下一个好点子\n☑ 把常用代码收进收藏\n☐ 早点休息，明天再看星星', kind: 'text', language: null, groupId: null, pinned: false },
  { title: 'MDN Web 文档', content: 'https://developer.mozilla.org/zh-CN/', kind: 'link', language: null, groupId: 2, pinned: false },
];
let demo: AppState = {
  clips: samples.map((clip, i) => ({ ...clip, keepForever: false, id: i + 1, createdAt: now - i * 1800000, updatedAt: now - i * 1800000, copyCount: i === 1 ? 12 : 0 })),
  groups: [{ id: 1, name: '灵感碎片', color: '#d9b780' }, { id: 2, name: '开发工具箱', color: '#87bab6' }, { id: 3, name: '设计收藏', color: '#b69bd7' }],
  settings: { paused: true, showStartup: true, stickerPack: 'auto', theme: 'dark', layout: 'grid', maxHistory: 0, retentionDays: 0, alwaysOnTop: false, shortcut: 'CommandOrControl+Shift+V' },
  clipboardError: null, shortcutError: null, windowError: null,
};
// ponytail: browser preview stays in memory; only the native app persists real history.
const listeners = new Set<() => void>();
function pruneDemo() {
  const { retentionDays, maxHistory } = demo.settings;
  const cutoff = retentionDays === 0 ? -Infinity : Date.now() - retentionDays * 86400000;
  let historyCount = 0;
  demo.clips = demo.clips.sort((a, b) => b.updatedAt - a.updatedAt || b.id - a.id).filter(clip =>
    clip.pinned || clip.keepForever || clip.groupId !== null || (clip.updatedAt >= cutoff && (maxHistory === 0 || ++historyCount <= maxHistory)));
}
let cleanupSequence = 0;
let pendingCleanup: { token: string; clips: Clip[] } | null = null;
const contentBytes = (clip: Clip) => new TextEncoder().encode(clip.content).length + new TextEncoder().encode(clip.title).length;
const cleanupFingerprint = (clip: Clip) => JSON.stringify([clip.id, clip.content, clip.title, clip.kind, clip.language, clip.pinned, clip.keepForever, clip.groupId, clip.createdAt, clip.updatedAt]);
function changed() { listeners.forEach((listener) => listener()); }
function id() { return Math.max(0, ...demo.clips.map((clip) => clip.id)) + 1; }
function requiredClip(clipId: number) { const clip = demo.clips.find((item) => item.id === clipId); if (!clip) throw new Error('记录不存在'); return clip; }
function validateContent(content: string) { if (!content.trim() || new TextEncoder().encode(content).length > 256 * 1024) throw new Error('内容不能为空，且不能超过 256 KB'); }
function classify(content: string): Clip['kind'] {
  if (/^#[\da-f]{3}(?:[\da-f]{3})?(?:[\da-f]{2})?$/i.test(content.trim())) return 'color';
  if (/^https?:\/\/\S+$/i.test(content.trim())) return 'link';
  if (/\b(function|const|let|fn |def |import |SELECT |class )/.test(content)) return 'code';
  return 'text';
}
async function invoke<T>(command: string, args?: Record<string, unknown>): Promise<T> { return (await import('@tauri-apps/api/core')).invoke<T>(command, args); }

export const api = {
  async setKeepForever(id: number, keepForever: boolean): Promise<void> {
    if (isDesktop) return invoke('set_keep_forever', { id, keepForever });
    requiredClip(id).keepForever = keepForever; changed();
  },
  async previewCleanup(filter: CleanupFilter): Promise<CleanupPreview> {
    if (isDesktop) return invoke('preview_cleanup', { filter });
    if (!Number.isSafeInteger(filter.minBytes) || filter.minBytes < 0 || !Number.isSafeInteger(filter.olderThanDays) || filter.olderThanDays < 0 || filter.olderThanDays > 36500 || !['all', 'text', 'link', 'code', 'color'].includes(filter.kind) || typeof filter.includeGrouped !== 'boolean') throw new Error('清理条件无效，请检查大小和天数。');
    const cutoff = Date.now() - filter.olderThanDays * 86400000;
    const clips = demo.clips.filter(c => !c.pinned && !c.keepForever && (filter.includeGrouped || c.groupId === null)
      && (filter.minBytes === 0 || contentBytes(c) > filter.minBytes)
      && (filter.olderThanDays === 0 || c.updatedAt < cutoff) && (filter.kind === 'all' || c.kind === filter.kind));
    const token = `demo-${++cleanupSequence}`;
    pendingCleanup = { token, clips: structuredClone(clips) };
    return { token, totalCount: demo.clips.length, protectedCount: demo.clips.filter(c => c.pinned || c.keepForever).length,
      matchedCount: clips.length, contentBytes: clips.reduce((sum, c) => sum + contentBytes(c), 0), databaseBytes: 0, reusableBytes: 0,
      items: clips.map(c => ({ id: c.id, title: c.title || c.content.slice(0, 44), kind: c.kind, bytes: contentBytes(c), updatedAt: c.updatedAt })).sort((a, b) => b.bytes - a.bytes || a.id - b.id).slice(0, 100), truncated: clips.length > 100 };
  },
  async executeCleanup(token: string, compact: boolean): Promise<CleanupResult> {
    if (isDesktop) return invoke('execute_cleanup', { token, compact });
    const pending = pendingCleanup; pendingCleanup = null;
    if (!pending || pending.token !== token) throw new Error('预览已失效，请重新预览。');
    const current = new Map(demo.clips.map(c => [c.id, c]));
    if (pending.clips.some(c => !current.has(c.id) || cleanupFingerprint(current.get(c.id)!) !== cleanupFingerprint(c))) throw new Error('待清理记录已变化，请重新预览。');
    const targets = new Set(pending.clips.map(c => c.id));
    demo.clips = demo.clips.filter(c => !targets.has(c.id)); changed();
    return { deletedCount: pending.clips.length, contentBytes: pending.clips.reduce((sum, c) => sum + contentBytes(c), 0), databaseBytesBefore: 0, databaseBytesAfter: 0, releasedBytes: 0, warning: '浏览器演示仅整理内存样例，不会释放本机磁盘空间。' };
  },
  async getDataLocation(): Promise<DataLocation> {
    if (!isDesktop) throw new Error('请在桌面版查看和管理数据目录。');
    return invoke('get_data_location');
  },
  async chooseDataDirectory(mode: DataDirectoryMode): Promise<DataDirectoryPlan | null> {
    if (!isDesktop) throw new Error('请在桌面版选择数据目录。');
    return invoke('choose_data_directory', { mode });
  },
  async applyDataDirectory(mode: DataDirectoryMode, targetPath: string): Promise<DataLocation> {
    if (!isDesktop) throw new Error('请在桌面版迁移数据。');
    return invoke('apply_data_directory', { mode, targetPath });
  },
  async openDataDirectory(): Promise<void> {
    if (!isDesktop) throw new Error('请在桌面版打开数据目录。');
    return invoke('open_data_directory');
  },
  async restartApp(): Promise<void> {
    if (!isDesktop) throw new Error('请在桌面版重启应用。');
    return invoke('restart_app');
  },
  async handwritingAssets(): Promise<HandwritingAssets> {
    return isDesktop ? invoke('handwriting_assets') : { available: false, path: null, fonts: [], backgrounds: [], error: '月光笺的手写效果需要在桌面版中连接本机星笺应用。星码卡可直接在此制作。' };
  },
  async chooseHandwritingApp(): Promise<HandwritingAssets> {
    return isDesktop ? invoke('choose_handwriting_app') : api.handwritingAssets();
  },
  async renderHandwriting(options: HandwritingOptions): Promise<string> {
    if (!isDesktop) throw new Error('请在桌面版中生成月光笺。');
    return invoke('render_handwriting', { options });
  },
  async getState(): Promise<AppState> { return isDesktop ? invoke('get_state') : structuredClone(demo); },
  async addClip(content: string, title: string, groupId: number | null, keepForever = false): Promise<Clip> {
    if (isDesktop) return invoke('add_clip', { content, title, groupId, keepForever });
    validateContent(content);
    const existing = demo.clips.find((clip) => clip.content === content);
    if (existing) {
      existing.updatedAt = Date.now();
      existing.keepForever ||= keepForever;
      if (title.trim()) existing.title = title.trim();
      if (groupId !== null) existing.groupId = groupId;
      pruneDemo(); changed(); return structuredClone(existing);
    }
    const clip: Clip = { id: id(), content, title, groupId, kind: classify(content), language: null, pinned: false, keepForever, createdAt: Date.now(), updatedAt: Date.now(), copyCount: 0 };
    demo.clips.unshift(clip); pruneDemo(); changed(); return structuredClone(clip);
  },
  async updateClip(id: number, title: string, groupId: number | null, pinned: boolean): Promise<void> {
    if (isDesktop) return invoke('update_clip', { id, title, groupId, pinned });
    Object.assign(requiredClip(id), { title: title.trim(), groupId, pinned }); changed();
  },
  async deleteClip(id: number): Promise<void> { if (isDesktop) return invoke('delete_clip', { id }); if (requiredClip(id).keepForever) throw new Error('请先取消这条记录的永久保留，再删除。'); demo.clips = demo.clips.filter((clip) => clip.id !== id); pendingCleanup = null; changed(); },
  async copyClip(id: number): Promise<void> {
    if (isDesktop) return invoke('copy_clip', { id });
    const clip = requiredClip(id); await navigator.clipboard.writeText(clip.content); clip.copyCount++; changed();
  },
  async createGroup(name: string, color: string): Promise<Group> {
    if (isDesktop) return invoke('create_group', { name, color });
    if (!name.trim() || name.length > 40 || !/^#[\da-f]{6}$/i.test(color)) throw new Error('分组名称或颜色无效');
    if (demo.groups.some((group) => group.name === name.trim())) throw new Error('分组名称已存在');
    const group = { id: Math.max(0, ...demo.groups.map((g) => g.id)) + 1, name: name.trim(), color: color.trim() };
    demo.groups.push(group); changed(); return structuredClone(group);
  },
  async deleteGroup(id: number): Promise<void> { if (isDesktop) return invoke('delete_group', { id }); demo.groups = demo.groups.filter((g) => g.id !== id); demo.clips.forEach((c) => { if (c.groupId === id) c.groupId = null; }); changed(); },
  async saveSettings(settings: Settings): Promise<void> {
    if (isDesktop) return invoke('save_settings', { settings });
    if (typeof settings.paused !== 'boolean' || typeof settings.alwaysOnTop !== 'boolean' || typeof settings.showStartup !== 'boolean'
      || !['dark', 'light', 'forest', 'rose', 'system'].includes(settings.theme) || !['grid', 'list', 'horizontal'].includes(settings.layout)
      || !['auto', 'none', 'cosmos', 'cats', 'puppies', 'garden'].includes(settings.stickerPack)
      || !Number.isSafeInteger(settings.maxHistory) || (settings.maxHistory !== 0 && (settings.maxHistory < 50 || settings.maxHistory > 10000))
      || ![0, 1, 7, 30, 90, 180, 365].includes(settings.retentionDays)
      || typeof settings.shortcut !== 'string' || !settings.shortcut.trim() || settings.shortcut.length > 80) {
      throw new Error('设置无效：请检查布局、历史上限、保留时长与快捷键。');
    }
    demo.settings = { ...settings }; pruneDemo(); changed();
  },
  async clearHistory(): Promise<void> { if (isDesktop) return invoke('clear_history'); demo.clips = demo.clips.filter((clip) => clip.pinned || clip.keepForever); pendingCleanup = null; changed(); },
  async exportBackup(): Promise<string> {
    if (isDesktop) return invoke('export_backup');
    const data = JSON.stringify({ format: 'starclip', version: 1, exportedAt: Date.now(), clips: demo.clips, groups: demo.groups }, null, 2);
    if (new TextEncoder().encode(data).length > 20 * 1024 * 1024 || demo.clips.length > 10000 || demo.groups.length > 1000) throw new Error('数据超过单个备份支持的范围（20 MB / 10000 条 / 1000 组）');
    return data;
  },
  async importBackup(data: string): Promise<number> {
    if (isDesktop) return invoke('import_backup', { data });
    if (new TextEncoder().encode(data).length > 20 * 1024 * 1024) throw new Error('备份不能超过 20 MB');
    const parsed: unknown = JSON.parse(data);
    if (!parsed || typeof parsed !== 'object') throw new Error('备份格式无效');
    const backup = parsed as { format?: unknown; version?: unknown; clips?: unknown; groups?: unknown };
    if (backup.format !== 'starclip' || backup.version !== 1 || !Array.isArray(backup.clips) || !Array.isArray(backup.groups) || backup.clips.length > 10000 || backup.groups.length > 1000) throw new Error('不支持的备份格式或条目过多');
    const next = structuredClone(demo), groupMap = new Map<number, number>();
    for (const group of backup.groups as Group[]) {
      if (!group || !Number.isSafeInteger(group.id) || typeof group.name !== 'string' || !group.name.trim() || group.name.length > 40 || !/^#[\da-f]{6}$/i.test(group.color)) throw new Error('备份分组无效');
      let match = next.groups.find((g) => g.name === group.name);
      if (!match) { match = { ...group, id: Math.max(0, ...next.groups.map((g) => g.id)) + 1 }; next.groups.push(match); }
      groupMap.set(group.id, match.id);
    }
    let added = 0;
    for (const clip of backup.clips as Clip[]) {
      if (!clip || typeof clip.content !== 'string' || typeof clip.title !== 'string' || clip.title.length > 200 || typeof clip.pinned !== 'boolean' || !Number.isFinite(clip.createdAt) || !Number.isFinite(clip.updatedAt) || !Number.isSafeInteger(clip.copyCount) || clip.copyCount < 0) throw new Error('备份条目无效');
      validateContent(clip.content);
      if (clip.keepForever !== undefined && typeof clip.keepForever !== 'boolean') throw new Error('备份保留标记无效');
      if (clip.language != null && !languages.some(([key]) => key === clip.language)) throw new Error('未知的代码语言');
      if (clip.groupId != null && !groupMap.has(clip.groupId)) throw new Error('分组引用无效');
      const existing = next.clips.find(c => c.content === clip.content);
      if (existing) {
        existing.keepForever ||= clip.keepForever ?? false;
        existing.pinned ||= clip.pinned;
        existing.groupId ??= clip.groupId == null ? null : groupMap.get(clip.groupId)!;
        if (!existing.title) existing.title = clip.title;
        existing.language ??= clip.language ?? null;
        if (existing.language) existing.kind = 'code';
        continue;
      }
      next.clips.push({ ...clip, keepForever: clip.keepForever ?? false, id: Math.max(0, ...next.clips.map((c) => c.id)) + 1, groupId: clip.groupId == null ? null : groupMap.get(clip.groupId)!, language: clip.language ?? null, kind: clip.language ? 'code' : classify(clip.content) }); added++;
    }
    // Import preserves the complete backup; only explicit capture/settings changes apply retention.
    demo = next; pendingCleanup = null; changed(); return added;
  },
  async saveBackup(): Promise<boolean> {
    if (isDesktop) return invoke('save_backup');
    download(new Blob([await api.exportBackup()], { type: 'application/json' }), 'starclip.backup.json'); return true;
  },
  async saveCode(id: number | null, title: string, content: string, language: string, groupId: number | null): Promise<Clip> {
    if (isDesktop) return invoke('save_code', { id, title, content, language, groupId });
    validateContent(content);
    if (!languages.some(([key]) => key === language)) throw new Error('未知的代码语言');
    const conflict = demo.clips.find((clip) => clip.content === content && clip.id !== id);
    if (conflict) throw new Error('相同内容已存在，请编辑已有记录');
    const actual: Clip = id == null
      ? { id: Math.max(0, ...demo.clips.map(clip => clip.id)) + 1, content, title: title.trim(), groupId, kind: 'code', language, pinned: true, keepForever: false, createdAt: Date.now(), updatedAt: Date.now(), copyCount: 0 }
      : requiredClip(id);
    Object.assign(actual, { title: title.trim(), content, language, kind: 'code', groupId, pinned: true, updatedAt: Date.now() });
    if (id == null) demo.clips.unshift(actual);
    pruneDemo(); changed(); return structuredClone(actual);
  },
  async saveExport(data: string, format: 'md' | 'png', filename: string): Promise<boolean> {
    if (isDesktop) return invoke('save_export', { data, format, filename });
    const blob = format === 'md' ? new Blob([data], { type: 'text/markdown;charset=utf-8' }) : new Blob([Uint8Array.from(atob(data), (c) => c.charCodeAt(0))], { type: 'image/png' });
    download(blob, filename); return true;
  },
  async hideWindow(): Promise<void> { if (isDesktop) return invoke('hide_window'); },
  async onChange(callback: () => void): Promise<() => void> {
    if (isDesktop) return (await import('@tauri-apps/api/event')).listen('clips-changed', callback);
    listeners.add(callback); return () => { listeners.delete(callback); };
  },
};
