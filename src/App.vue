<script setup lang="ts">
import { computed, nextTick, onMounted, onUnmounted, ref, watch } from 'vue'
import { api, isDesktop } from './api'
import type { AppState, Clip, Settings } from './types'
import Icon from './components/Icon.vue'
import CodeStudio from './components/CodeStudio.vue'
import StartupScreen from './components/StartupScreen.vue'
import DataSettings from './components/DataSettings.vue'
import CleanupPanel from './components/CleanupPanel.vue'
import StickerScene from './components/StickerScene.vue'
import OnboardingGuide from './components/OnboardingGuide.vue'

const state = ref<AppState>({ onboardingCompleted: true, clips: [], groups: [], settings: { paused: true, theme: 'dark', layout: 'grid', maxHistory: 0, retentionDays: 0, alwaysOnTop: false, showStartup: true, stickerPack: 'auto', shortcut: 'CommandOrControl+Shift+V' }, clipboardError: null, shortcutError: null, windowError: null })
const loading = ref(true)
const busy = ref(false)
const dataBusy = ref(false)
const startupVisible = ref(false)
const firstGuidePending = ref(false)
const guideVisible = ref(false)
const guideBusy = ref(false)
const guideError = ref('')
const tutorialButton = ref<HTMLButtonElement>()
const filter = ref('all')
const query = ref('')
const selected = ref<number | null>(null)
const searchInput = ref<HTMLInputElement>()
const fileInput = ref<HTMLInputElement>()
const modal = ref<'' | 'clip' | 'group' | 'settings' | 'clear' | 'delete-group'>('')
const dialog = ref<HTMLDialogElement>()
const editing = ref<Clip | null>(null)
const studioOpen = ref(false)
const studioClip = ref<Clip | null>(null)
const clipTitle = ref('')
const clipContent = ref('')
const clipGroup = ref<number | null>(null)
const clipPinned = ref(false)
const clipKept = ref(false)
const groupName = ref('')
const groupColor = ref('#8b93d9')
const settingsDraft = ref<Settings>({ ...state.value.settings })
const settingsSaving = ref(false)
const settingsTabs = [
  { key: 'appearance', label: '外观', icon: 'moon' },
  { key: 'habits', label: '使用习惯', icon: 'sliders' },
  { key: 'privacy', label: '记录与隐私', icon: 'shield' },
  { key: 'data', label: '数据管理', icon: 'folder' },
  { key: 'help', label: '帮助', icon: 'spark' },
] as const
type SettingsTab = typeof settingsTabs[number]['key']
const settingsTab = ref<SettingsTab>('appearance')
const settingsScroll = ref<HTMLElement>()
const dataTabVisited = ref(false)
const settingsDirty = computed(() => (Object.keys(settingsDraft.value) as Array<keyof Settings>).some(key => settingsDraft.value[key] !== state.value.settings[key]))
const toast = ref('')
const toastError = ref(false)
const copied = ref<number | null>(null)
let toastTimer: ReturnType<typeof setTimeout>
let copyTimer: ReturnType<typeof setTimeout>
let unsubscribe: (() => void) | undefined
const colors = ['#8b93d9', '#71baad', '#d7b678', '#d98eaa', '#89afd5', '#b3a192']
const kindNames = { text: '文本', link: '链接', code: '代码', color: '色彩' }
const navItems = [{ key: 'all', name: '全部记录', icon: 'grid' }, { key: 'pinned', name: '星标收藏', icon: 'star' }, { key: 'kept', name: '永久保留', icon: 'shield' }, { key: 'link', name: '链接', icon: 'link' }, { key: 'code', name: '星码集', icon: 'code' }]
const visible = computed(() => {
  const q = query.value.trim().toLocaleLowerCase()
  return state.value.clips.filter(c => {
    const inFilter = filter.value === 'all' || (filter.value === 'pinned' && c.pinned) || (filter.value === 'kept' && c.keepForever) || filter.value === c.kind || filter.value === `group-${c.groupId}`
    return inFilter && (!q || `${c.title}\n${c.content}`.toLocaleLowerCase().includes(q))
  }).sort((a, b) => b.updatedAt - a.updatedAt)
})
const sectionName = computed(() => navItems.find(n => n.key === filter.value)?.name || state.value.groups.find(g => `group-${g.id}` === filter.value)?.name || '全部记录')
const currentGroup = computed(() => state.value.groups.find(g => `group-${g.id}` === filter.value))
const selectedIndex = computed(() => visible.value.findIndex(c => c.id === selected.value))
const dateLabel = new Intl.DateTimeFormat('zh-CN', { month: 'long', day: 'numeric', weekday: 'long' }).format(new Date())
const isMac = navigator.platform.toLowerCase().includes('mac')
const modKey = isMac ? '⌘' : 'Ctrl'
const shortcutLabel = computed(() => state.value.settings.shortcut.replace(/CommandOrControl|CmdOrCtrl/g, isMac ? '⌘' : 'Ctrl').replace(/Shift/g, '⇧').replaceAll('+', ' '))
const themeOptions = [
  { value: 'dark', name: '星夜', description: '深邃夜空' },
  { value: 'light', name: '月白', description: '清透微光' },
  { value: 'forest', name: '青苔', description: '轻松绿意' },
  { value: 'rose', name: '暮樱', description: '柔和暖粉' },
  { value: 'system', name: '跟随系统', description: '随系统切换' },
] as const
const stickerOptions = [
  { value: 'auto', name: '随主题', description: '为每种底色配一份小可爱' },
  { value: 'cosmos', name: '星月漫游', description: '月亮、星星与小行星' },
  { value: 'cats', name: '奶油猫咪', description: '蝴蝶结与软乎乎的小猫' },
  { value: 'puppies', name: '云朵小狗', description: '线条小狗与小爪印' },
  { value: 'garden', name: '花园来信', description: '花朵、嫩叶与小盆栽' },
  { value: 'none', name: '清爽留白', description: '关闭装饰，专注内容' },
] as const
const systemTheme = window.matchMedia('(prefers-color-scheme: dark)')
const resolvedTheme = ref<Exclude<Settings['theme'], 'system'>>('dark')
function applyTheme() { resolvedTheme.value = state.value.settings.theme === 'system' ? (systemTheme.matches ? 'dark' : 'light') : state.value.settings.theme; document.documentElement.dataset.theme = resolvedTheme.value }
watch(() => state.value.settings.theme, applyTheme)
watch(filter, () => { selected.value = null; query.value = '' })
watch(visible, clips => { if (!clips.some(c => c.id === selected.value)) selected.value = clips[0]?.id ?? null })
watch(modal, async value => { await nextTick(); if (value && !dialog.value?.open) dialog.value?.showModal(); if (!value && dialog.value?.open) dialog.value?.close() })
function notify(message: string, error = false) { clearTimeout(toastTimer); toast.value = message; toastError.value = error; toastTimer = setTimeout(() => { toast.value = '' }, 3600) }
async function refresh() { try { state.value = await api.getState(); applyTheme() } catch (error) { notify(String(error), true) } finally { loading.value = false } }
async function run(action: () => Promise<unknown>, success?: string) { if (busy.value || dataBusy.value) return; busy.value = true; try { await action(); await refresh(); if (success) notify(success) } catch (error) { notify(String(error), true) } finally { busy.value = false } }
function count(key: string) { return state.value.clips.filter(c => key === 'all' || (key === 'pinned' && c.pinned) || (key === 'kept' && c.keepForever) || c.kind === key).length }
function groupFor(clip: Clip) { return state.value.groups.find(g => g.id === clip.groupId) }
function relativeTime(time: number) { const minutes = Math.max(0, Math.floor((Date.now() - time) / 60000)); if (minutes < 1) return '刚刚'; if (minutes < 60) return `${minutes} 分钟前`; if (minutes < 1440) return `${Math.floor(minutes / 60)} 小时前`; if (minutes < 10080) return `${Math.floor(minutes / 1440)} 天前`; return new Date(time).toLocaleDateString('zh-CN', { month: 'short', day: 'numeric' }) }
function titleFor(clip: Clip) { return clip.title || clip.content.trim().split('\n')[0]?.slice(0, 44) || '未命名片段' }
function linkHost(content: string) { try { return new URL(content.trim()).hostname } catch { return '网页链接' } }
function colorValue(content: string) { return /^#[\da-f]{3,8}$/i.test(content.trim()) ? content.trim() : '#8b93d9' }
async function copy(clip: Clip) { try { await api.copyClip(clip.id); copied.value = clip.id; clearTimeout(copyTimer); copyTimer = setTimeout(() => { copied.value = null }, 1800); notify('已复制，切换到目标应用即可粘贴'); await refresh() } catch (error) { notify(String(error), true) } }
async function togglePin(clip: Clip) { await run(() => api.updateClip(clip.id, clip.title, clip.groupId, !clip.pinned), clip.pinned ? '已取消星标' : '已收入星标收藏') }
async function toggleKept(clip: Clip) { await run(() => api.setKeepForever(clip.id, !clip.keepForever), clip.keepForever ? '已取消永久保留' : '已永久保留，不参与清理') }
function openCleanup() { if (!busy.value && !dataBusy.value) modal.value = 'clear' }
function openClip(clip?: Clip) { editing.value = clip ?? null; clipTitle.value = clip?.title || ''; clipContent.value = clip?.content || ''; clipGroup.value = clip?.groupId ?? currentGroup.value?.id ?? null; clipPinned.value = clip?.pinned ?? false; clipKept.value = clip?.keepForever ?? false; modal.value = 'clip' }
function openStudio(clip: Clip | null = null) { modal.value = ''; studioClip.value = clip; studioOpen.value = true }
async function saveClip() {
  if (!clipContent.value.trim()) return notify('先写下一点内容吧', true)
  await run(async () => {
    if (editing.value) {
      // Protect first: automatic cleanup may run between these two native commands.
      if (clipKept.value && !editing.value.keepForever) await api.setKeepForever(editing.value.id, true)
      await api.updateClip(editing.value.id, clipTitle.value.trim(), clipGroup.value, clipPinned.value)
      if (!clipKept.value && editing.value.keepForever) await api.setKeepForever(editing.value.id, false)
    }
    else { const clip = await api.addClip(clipContent.value, clipTitle.value.trim(), clipGroup.value, clipKept.value); if (clipPinned.value) await api.updateClip(clip.id, clip.title, clipGroup.value, true) }
    modal.value = ''
  }, editing.value ? '片段已更新' : '新片段已收好')
}
function openGroup() { groupName.value = ''; groupColor.value = colors[state.value.groups.length % colors.length]!; modal.value = 'group' }
async function saveGroup() { if (!groupName.value.trim()) return notify('请为星座起一个名字', true); await run(async () => { const group = await api.createGroup(groupName.value.trim(), groupColor.value); filter.value = `group-${group.id}`; modal.value = '' }, '分组已创建') }
function closeModal() { if (!dataBusy.value && !busy.value) modal.value = '' }
function replayStartup() { if (!dataBusy.value) startupVisible.value = true }
function openSettings() { settingsDraft.value = { ...state.value.settings }; dataTabVisited.value = settingsTab.value === 'data'; modal.value = 'settings' }
function changeSettingsTab(tab: SettingsTab) {
  if (busy.value || dataBusy.value) return
  settingsTab.value = tab
  if (tab === 'data') dataTabVisited.value = true
  void nextTick(() => settingsScroll.value?.scrollTo({ top: 0 }))
}
function settingsTabKeyboard(event: KeyboardEvent) {
  if (event.altKey || event.ctrlKey || event.metaKey || event.isComposing || !['ArrowLeft', 'ArrowRight', 'Home', 'End'].includes(event.key)) return
  event.preventDefault()
  if (busy.value || dataBusy.value) return
  const index = settingsTabs.findIndex(tab => tab.key === settingsTab.value)
  const next = event.key === 'Home' ? 0 : event.key === 'End' ? settingsTabs.length - 1 : (index + (event.key === 'ArrowRight' ? 1 : -1) + settingsTabs.length) % settingsTabs.length
  const tab = settingsTabs[next]!.key
  changeSettingsTab(tab)
  void nextTick(() => dialog.value?.querySelector<HTMLButtonElement>(`#settings-tab-${tab}`)?.focus())
}
function finishStartup() {
  startupVisible.value = false
  if (firstGuidePending.value) { firstGuidePending.value = false; guideVisible.value = true }
}
function openGuide() {
  if (busy.value || dataBusy.value) return
  guideError.value = ''; guideVisible.value = true
}
function dismissGuide() {
  guideVisible.value = false; guideError.value = ''
  void nextTick(() => tutorialButton.value?.focus())
}
async function finishGuide() {
  if (guideBusy.value) return
  guideBusy.value = true; guideError.value = ''
  try {
    if (!state.value.onboardingCompleted) await api.completeOnboarding()
    state.value.onboardingCompleted = true
    dismissGuide()
  } catch (error) { guideError.value = String(error) }
  finally { guideBusy.value = false }
}
async function saveSettings() {
  if (busy.value || dataBusy.value || !settingsDirty.value) return
  settingsSaving.value = true
  try { await run(() => api.saveSettings({ ...settingsDraft.value }), '偏好设置已保存') }
  finally { settingsSaving.value = false }
}
async function toggleCapture() { await run(() => api.saveSettings({ ...state.value.settings, paused: !state.value.settings.paused }), state.value.settings.paused ? '已开启记录，从下一次复制开始' : '已暂停记录') }
async function changeLayout(layout: Settings['layout']) { await run(() => api.saveSettings({ ...state.value.settings, layout })) }
async function changeTheme(event: Event) {
  const field = event.target as HTMLSelectElement
  const theme = field.value as Settings['theme']
  await run(() => api.saveSettings({ ...state.value.settings, theme }))
  field.value = state.value.settings.theme
}
async function deleteEditing() { const id = editing.value?.id; if (id === undefined) return; await run(async () => { await api.deleteClip(id); modal.value = '' }, '片段已删除') }
async function exportBackup() { await run(async () => { if (isDesktop) { if (await api.saveBackup()) notify('备份已保存') } else { const url = URL.createObjectURL(new Blob([await api.exportBackup()], { type: 'application/json' })); const a = document.createElement('a'); a.href = url; a.download = `starclip-${new Date().toISOString().slice(0, 10)}.json`; a.click(); setTimeout(() => URL.revokeObjectURL(url), 1000); notify('备份已导出') } }) }
async function importBackup(event: Event) { const input = event.target as HTMLInputElement; const file = input.files?.[0]; if (!file) return; if (file.size > 20 * 1024 * 1024) { notify('备份文件过大，最大支持 20 MB', true); input.value = ''; return }; await run(async () => { const count = await api.importBackup(await file.text()); notify(`已导入 ${count} 条记录`) }); input.value = '' }
async function deleteCurrentGroup() { const id = currentGroup.value?.id; if (id === undefined) return; await run(async () => { await api.deleteGroup(id); filter.value = 'all'; modal.value = '' }, '分组已删除，片段仍保留在全部记录中') }
function moveSelection(delta: number) {
  if (!visible.value.length) return
  const index = Math.min(visible.value.length - 1, Math.max(0, Math.max(0, selectedIndex.value) + delta))
  selected.value = visible.value[index]!.id
  const card = document.querySelector<HTMLElement>(`[data-clip-id="${selected.value}"]`)
  card?.focus({ preventScroll: true })
  card?.scrollIntoView({ block: 'nearest', inline: 'nearest' })
}
function keyboard(event: KeyboardEvent) {
  const target = event.target as HTMLElement
  const typing = target.matches('input, textarea, select') || target.isContentEditable
  const modifier = event.metaKey || event.ctrlKey
  if (guideVisible.value) { if (modifier && event.key.toLowerCase() === 's') event.preventDefault(); return }
  if (startupVisible.value || studioOpen.value || event.isComposing) return
  if (modal.value === 'settings' && modifier && !event.altKey && !event.shiftKey && event.key.toLowerCase() === 's') {
    event.preventDefault()
    if (!busy.value && !dataBusy.value && settingsDirty.value) dialog.value?.querySelector<HTMLFormElement>('.settings-form')?.requestSubmit()
    return
  }
  if (modal.value) return
  if (modifier && event.key.toLowerCase() === 'k') { event.preventDefault(); searchInput.value?.focus(); return }
  if (typing) return
  if (modifier && event.shiftKey && !event.altKey && ['ArrowLeft', 'ArrowRight'].includes(event.key)) {
    event.preventDefault()
    const sections = [...navItems.map(item => item.key), ...state.value.groups.map(group => `group-${group.id}`)]
    const direction = event.key === 'ArrowRight' ? 1 : -1
    filter.value = sections[(Math.max(0, sections.indexOf(filter.value)) + direction + sections.length) % sections.length]!
    void nextTick(() => moveSelection(0))
    return
  }
  if (target.closest('button, a')) return
  if (modifier && !event.shiftKey && !event.altKey && /^[1-9]$/.test(event.key)) {
    const clip = visible.value[Number(event.key) - 1]
    if (clip) { event.preventDefault(); void copy(clip) }
    return
  }
  if (modifier || event.altKey || event.shiftKey) return
  if (['ArrowLeft', 'ArrowRight', 'ArrowUp', 'ArrowDown'].includes(event.key) && visible.value.length) {
    event.preventDefault()
    const columns = state.value.settings.layout === 'grid' ? (window.innerWidth > 1150 ? 3 : 2) : 1
    moveSelection(event.key === 'ArrowDown' ? columns : event.key === 'ArrowUp' ? -columns : event.key === 'ArrowRight' ? 1 : -1)
  } else if (event.key === 'Enter' && selected.value !== null) {
    const clip = visible.value.find(c => c.id === selected.value)
    if (clip) { event.preventDefault(); void copy(clip) }
  } else if (event.key === 'Escape') { if (query.value) query.value = ''; else if (isDesktop) void api.hideWindow() }
}
onMounted(async () => { await refresh(); firstGuidePending.value = !state.value.onboardingCompleted; startupVisible.value = state.value.settings.showStartup; if (!startupVisible.value) finishStartup(); unsubscribe = await api.onChange(() => { void refresh() }); window.addEventListener('keydown', keyboard); systemTheme.addEventListener('change', applyTheme) })
onUnmounted(() => { unsubscribe?.(); clearTimeout(toastTimer); clearTimeout(copyTimer); window.removeEventListener('keydown', keyboard); systemTheme.removeEventListener('change', applyTheme) })
</script>

<template>
  <StartupScreen v-if="startupVisible" @done="finishStartup" />
  <OnboardingGuide v-if="guideVisible" :busy="guideBusy" :error="guideError" :theme="resolvedTheme" @finish="finishGuide" @dismiss="dismissGuide" />
  <div class="app-shell" :inert="startupVisible || guideVisible">
    <aside class="sidebar">
      <a class="brand" href="#" @click.prevent="filter = 'all'" aria-label="星笺首页"><span class="brand-mark"><Icon name="spark" :size="27" /><i /></span><span><strong>星笺<span class="brand-dot">.</span></strong><small>STARCLIP</small></span></a>
      <div class="workspace-label"><span class="workspace-orbit" />我的星空 <span class="local-badge">LOCAL</span></div>
      <nav class="main-nav" aria-label="记录分类"><button v-for="item in navItems" :key="item.key" :class="['nav-item', { active: filter === item.key }]" @click="filter = item.key" :aria-current="filter === item.key ? 'page' : undefined" :title="item.key === 'code' ? '星码集 · 收藏与管理代码片段' : item.name"><Icon :name="item.icon" /><span>{{ item.name }}</span><span class="nav-count">{{ count(item.key) }}</span></button></nav>
      <div class="group-heading"><span>我的星座</span><button class="icon-button" title="新建分组" aria-label="新建分组" @click="openGroup"><Icon name="plus" :size="16" /></button></div>
      <nav class="group-nav" aria-label="分组"><button v-for="group in state.groups" :key="group.id" :class="['nav-item', { active: filter === `group-${group.id}` }]" @click="filter = `group-${group.id}`"><span class="group-dot" :style="{ background: group.color }" /><span class="truncate">{{ group.name }}</span><span class="nav-count">{{ state.clips.filter(c => c.groupId === group.id).length }}</span></button><button v-if="!state.groups.length" class="empty-group" @click="openGroup"><Icon name="plus" :size="14" />给灵感一个小分组</button></nav>
      <div class="sidebar-bottom"><div class="night-note"><StickerScene :pack="state.settings.stickerPack" :theme="resolvedTheme" placement="sidebar" /><p>让散落的灵感，<br>在这里连成星座。</p><span>YOUR LITTLE UNIVERSE</span></div><button class="capture-toggle" :title="state.settings.paused ? '当前已暂停，点击后从下一次复制开始记录' : '点击暂停剪贴记录'" :class="{ paused: state.settings.paused }" @click="toggleCapture" :disabled="busy || dataBusy"><span class="status-dot" /><span>{{ state.settings.paused ? '开启剪贴记录' : '正在拾取灵感' }}</span><Icon :name="state.settings.paused ? 'play' : 'pause'" :size="13" /></button><button class="nav-item settings-nav" @click="openSettings"><Icon name="sliders" /><span>偏好设置</span><span class="version">v0.1.0</span></button></div>
    </aside>
    <main class="main-panel">
      <header class="topbar"><div class="breadcrumb"><Icon name="spark" :size="15" /><span>我的星空</span><Icon name="chevron" :size="12" /><strong>{{ sectionName }}</strong></div><div class="topbar-right"><StickerScene class="header-stickers" :pack="state.settings.stickerPack" :theme="resolvedTheme" placement="header" /><span v-if="!isDesktop" class="demo-badge">交互演示 · 示例数据</span><span class="date-label">{{ dateLabel }}</span><label class="theme-picker" :title="'当前主题：' + themeOptions.find(option => option.value === state.settings.theme)?.name"><span class="theme-picker-dot" aria-hidden="true" /><select :value="state.settings.theme" aria-label="切换主题" :disabled="busy || dataBusy" @change="changeTheme"><option v-for="option in themeOptions" :key="option.value" :value="option.value">{{ option.name }}</option></select></label></div></header>
      <div :class="['main-scroll', { 'horizontal-layout': state.settings.layout === 'horizontal' }]">
        <div v-if="state.clipboardError || state.shortcutError || state.windowError" class="error-banner" role="alert"><Icon name="alert" :size="16" /><span>{{ state.clipboardError || state.shortcutError || state.windowError }}</span><button @click="openSettings">查看设置</button></div>
        <section class="collection" aria-label="剪贴板记录">
          <div class="collection-heading"><div><h2>{{ sectionName }} <span>{{ visible.length }}</span></h2><p>{{ query ? `正在查找「${query}」` : filter === 'pinned' ? '那些想要再次遇见的片段' : filter === 'kept' ? '认真留下的，时间也不会带走' : filter === 'code' ? '代码收藏 · 把常用的几行，收进自己的星空' : currentGroup ? '把相近的灵感，放在同一片星空' : '好想法，总会再次闪光' }}</p></div><div class="collection-actions"><button class="button subtle cleanup-entry" title="星尘整理 · 按大小、时间与类型清理历史" :disabled="busy || dataBusy" @click="openCleanup"><Icon name="spark" :size="16" />星尘整理</button><button v-if="currentGroup" class="icon-button" title="删除当前分组" aria-label="删除当前分组" @click="modal = 'delete-group'"><Icon name="trash" :size="16" /></button><button class="button subtle code-create" title="星绘坊 · 把文字与代码做成卡片" @click="openStudio()"><Icon name="code" :size="16" />星绘坊</button><button class="button primary" @click="openClip()"><Icon name="plus" :size="16" />新建片段</button></div></div>
          <div class="toolbar"><label class="search-box"><Icon name="search" :size="18" /><input ref="searchInput" v-model="query" placeholder="搜索你收藏的每一点灵感…" aria-label="搜索记录" /><button v-if="query" class="icon-button" @click="query = ''" title="清空搜索" aria-label="清空搜索"><Icon name="close" :size="14" /></button><kbd v-else>{{ modKey }} K</kbd></label><div class="layout-switch" role="group" aria-label="显示方式"><button :class="{ selected: state.settings.layout === 'grid' }" title="网格视图" aria-label="网格视图" :aria-pressed="state.settings.layout === 'grid'" @click="changeLayout('grid')"><Icon name="grid" :size="17" /></button><button :class="{ selected: state.settings.layout === 'list' }" title="列表视图" aria-label="列表视图" :aria-pressed="state.settings.layout === 'list'" @click="changeLayout('list')"><Icon name="list" :size="17" /></button><button :class="{ selected: state.settings.layout === 'horizontal' }" title="流星栏 · 横向快取" aria-label="流星栏 · 横向快取" :aria-pressed="state.settings.layout === 'horizontal'" @click="changeLayout('horizontal')"><Icon name="list" :size="17" class="horizontal-view-icon" /></button></div></div>
          <div v-if="state.settings.layout === 'horizontal'" class="meteor-bar-heading"><div><Icon name="spark" :size="15" /><strong>流星栏</strong><span>横向浏览，快速取用 · ← / ↑ 上一条，→ / ↓ 下一条</span></div><div class="meteor-controls"><span>{{ visible.length ? selectedIndex + 1 : 0 }} / {{ visible.length }}</span><button class="icon-button" aria-label="上一条片段" title="上一条片段" :disabled="selectedIndex <= 0" @click="moveSelection(-1)"><Icon name="chevron" :size="15" class="previous-icon" /></button><button class="icon-button" aria-label="下一条片段" title="下一条片段" :disabled="selectedIndex < 0 || selectedIndex >= visible.length - 1" @click="moveSelection(1)"><Icon name="chevron" :size="15" /></button></div></div>
          <div class="results-label"><span>{{ query ? '搜索结果' : '最近收录' }}</span><span>按最近更新排序 <Icon name="clock" :size="12" /></span></div>
          <div v-if="loading" class="empty-state"><span class="loading-orbit" /><h3>正在点亮你的星空…</h3></div>
          <div v-else-if="!visible.length" class="empty-state"><div class="empty-illustration"><Icon :name="query ? 'search' : filter === 'pinned' ? 'star' : 'spark'" :size="38" /><i>✧</i><b>·</b></div><h3>{{ query ? '还没找到这颗星' : filter === 'pinned' ? '为喜欢的灵感点一颗星' : filter === 'kept' ? '给重要的灵感一个长久的家' : '你的第一颗灵感，还在路上' }}</h3><p>{{ query ? '换个关键词试试，或查看全部记录。' : filter === 'pinned' ? '点击片段上的星星，它就会出现在这里。' : filter === 'kept' ? '点击片段上的盾牌，标记为永久保留。' : '复制一段文字，或亲手写下一个新片段。' }}</p><button class="button subtle" @click="query ? query = '' : openClip()">{{ query ? '清空搜索' : '写下第一条片段' }}<Icon name="arrow" :size="15" /></button></div>
          <div v-else :class="['clip-collection', state.settings.layout]" :aria-label="state.settings.layout === 'horizontal' ? '流星栏：横向片段列表' : undefined">
            <article v-for="(clip, index) in visible" :key="clip.id" :data-clip-id="clip.id" :class="['clip-card', `kind-${clip.kind}`, { 'is-selected': selected === clip.id, 'is-copied': copied === clip.id }]" tabindex="0" :aria-label="`${titleFor(clip)}，${kindNames[clip.kind]}，双击复制`" @click="selected = clip.id" @focus="selected = clip.id" @dblclick="copy(clip)" @keydown.enter="event => { if (event.target === event.currentTarget) { event.stopPropagation(); event.preventDefault(); copy(clip) } }">
              <div class="card-top"><span :class="['kind-icon', clip.kind]"><Icon :name="clip.kind" :size="16" /></span><span class="kind-label">{{ clip.language || kindNames[clip.kind] }}</span><div class="card-top-actions" @dblclick.stop><button :class="['icon-button', 'keep-button', { kept: clip.keepForever }]" :title="clip.keepForever ? '永久保留中 · 点击取消' : '永久保留，不参与清理'" :aria-label="`${clip.keepForever ? '取消永久保留' : '永久保留'} ${titleFor(clip)}`" :aria-pressed="clip.keepForever" @click.stop="toggleKept(clip)"><Icon name="shield" :size="15" /></button><button :class="['icon-button', 'pin-button', { pinned: clip.pinned }]" :title="clip.pinned ? '取消星标' : '收藏到星标'" :aria-label="clip.pinned ? '取消星标' : '收藏到星标'" :aria-pressed="clip.pinned" @click.stop="togglePin(clip)"><Icon name="star" :size="15" /></button><button class="icon-button edit-button" title="查看和编辑" aria-label="查看和编辑" @click.stop="openClip(clip)"><Icon name="more" :size="17" /></button></div></div>
              <div class="card-main" @click="selected = clip.id"><template v-if="clip.kind === 'color'"><div class="color-preview" :style="{ background: colorValue(clip.content) }"><span>{{ clip.content.trim() }}</span></div><h3>{{ titleFor(clip) }}</h3></template><template v-else><h3>{{ titleFor(clip) }}</h3><div v-if="clip.kind === 'link'" class="link-preview"><span class="link-favicon">{{ linkHost(clip.content)[0]?.toUpperCase() }}</span><div><strong>{{ linkHost(clip.content) }}</strong><span>{{ clip.content }}</span></div><Icon name="arrow" :size="15" /></div><pre v-else-if="clip.kind === 'code'" class="code-preview">{{ clip.content }}</pre><p v-else class="text-preview">{{ clip.content }}</p></template></div>
              <div class="card-bottom"><span v-if="groupFor(clip)" class="group-tag" :style="{ '--group-color': groupFor(clip)?.color }"><span />{{ groupFor(clip)?.name }}</span><span v-else class="time-label">{{ relativeTime(clip.updatedAt) }}</span><div class="card-copy-area" @dblclick.stop><kbd v-if="index < 9">{{ modKey }} {{ index + 1 }}</kbd><button :class="['copy-button', { copied: copied === clip.id }]" @click.stop="copy(clip)" :title="`复制${titleFor(clip)}`" :aria-label="`复制${titleFor(clip)}`"><Icon :name="copied === clip.id ? 'check' : 'copy'" :size="14" /><span>{{ copied === clip.id ? '已复制' : '复制' }}</span></button></div></div>
            </article>
          </div>
        </section>
        <div class="collection-end" v-if="visible.length"><span /> <Icon name="spark" :size="11" /> 每一点灵感，都有它的位置 <span /></div>
      </div>
      <footer class="statusbar"><span><span :class="['status-dot', { idle: state.settings.paused }]" />{{ state.settings.paused ? '记录已暂停' : '本地记录中' }}<span class="footer-divider">/</span>{{ state.clips.length }} 条记录</span><div><span><kbd>{{ state.settings.layout === 'horizontal' ? '← →' : '↑ ↓' }}</kbd> 选择</span><span><kbd>↵</kbd> 复制</span><span class="group-shortcut-footer"><kbd>{{ modKey }} ⇧ ← / →</kbd> 切换分类</span><span class="shortcut-footer"><kbd>{{ shortcutLabel }}</kbd> 呼出星笺</span></div></footer>
    </main>
    <dialog ref="dialog" :class="['modal', { 'settings-modal': modal === 'settings', 'cleanup-modal': modal === 'clear' }]" @cancel.prevent="closeModal" @close="modal = ''" @click="event => { if (event.target === dialog) closeModal() }">
      <div class="modal-content" v-if="modal">
        <div class="modal-heading"><div><div class="eyebrow">{{ modal === 'settings' ? 'MAKE IT YOURS' : modal === 'group' ? 'A NEW CONSTELLATION' : 'A LITTLE THOUGHT' }}</div><h2>{{ modal === 'clip' ? editing ? '这颗灵感的细节' : '留住一点灵感' : modal === 'group' ? '点亮一个新星座' : modal === 'settings' ? '偏好设置' : modal === 'clear' ? '星尘整理' : '删除这个分组？' }}</h2></div><button class="icon-button" aria-label="关闭弹窗" :disabled="busy || dataBusy" @click="closeModal"><Icon name="close" :size="20" /></button></div>
        <form v-if="modal === 'clip'" @submit.prevent="saveClip" class="clip-form">
          <label>片段名称 <span class="optional">选填</span><input v-model="clipTitle" maxlength="200" placeholder="为这颗灵感起个名字" autofocus /></label>
          <label>内容 <span v-if="editing" class="optional">原始记录 · 只读</span><textarea v-model="clipContent" :readonly="!!editing" placeholder="写下一段文字、一个链接，或者几行代码…" rows="7" required /><span class="field-note">{{ clipContent.length }} 个字符{{ editing ? ` · 收录于 ${new Date(editing.createdAt).toLocaleString('zh-CN')}` : '' }}</span></label>
          <div class="form-row"><label>所属星座<select v-model="clipGroup"><option :value="null">未分组</option><option v-for="group in state.groups" :value="group.id" :key="group.id">{{ group.name }}</option></select></label><label class="checkbox-label"><input type="checkbox" v-model="clipPinned" /><Icon name="star" :size="17" />加入星标收藏</label></div>
          <label class="checkbox-label keep-setting"><input type="checkbox" v-model="clipKept" /><Icon name="shield" :size="17" />永久保留，不参与清理</label><p v-if="editing?.keepForever" class="field-note">这条记录已受保护；如需删除，请先取消永久保留并保存。</p>
          <div class="modal-actions"><button v-if="editing" type="button" class="button danger-quiet" @click="deleteEditing" :disabled="busy || dataBusy || editing.keepForever"><Icon name="trash" :size="16" />删除</button><span class="spacer" /><button v-if="editing" type="button" class="button subtle" @click="openStudio(editing)"><Icon name="code" :size="15" />带入星绘坊</button><button type="submit" class="button primary" :disabled="busy || dataBusy || !clipContent.trim()">{{ editing ? '保存修改' : '收下这颗灵感' }}<Icon name="check" :size="16" /></button></div>
        </form>
        <form v-else-if="modal === 'group'" @submit.prevent="saveGroup"><p class="modal-intro">把常用片段放在一起，下次相遇更容易。</p><label>星座名称<input v-model="groupName" maxlength="40" placeholder="例如：工作碎片、值得一读" required autofocus /></label><label class="color-label">标记颜色<div class="color-choices"><button v-for="color in colors" :key="color" type="button" :style="{ background: color }" :class="{ chosen: groupColor === color }" :aria-label="`选择颜色 ${color}`" :aria-pressed="groupColor === color" @click="groupColor = color"><Icon v-if="groupColor === color" name="check" :size="16" /></button></div></label><div class="modal-actions"><button type="button" class="button subtle" :disabled="dataBusy" @click="closeModal">取消</button><button class="button primary" :disabled="busy || dataBusy || !groupName.trim()">创建分组<Icon name="plus" :size="16" /></button></div></form>
        <form v-else-if="modal === 'settings'" @submit.prevent="saveSettings" class="settings-form">
          <nav class="settings-tabs" role="tablist" aria-label="偏好分类" @keydown="settingsTabKeyboard">
            <button v-for="tab in settingsTabs" :key="tab.key" :id="`settings-tab-${tab.key}`" type="button" role="tab" :aria-selected="settingsTab === tab.key" :aria-controls="`settings-panel-${tab.key}`" :tabindex="settingsTab === tab.key ? 0 : -1" :disabled="busy || dataBusy" @click="changeSettingsTab(tab.key)"><Icon :name="tab.icon" :size="15" /><span>{{ tab.label }}</span></button>
          </nav>
          <div ref="settingsScroll" class="settings-scroll">
            <section v-show="settingsTab === 'appearance'" id="settings-panel-appearance" role="tabpanel" aria-labelledby="settings-tab-appearance" class="settings-panel">
              <fieldset class="settings-section preferences-boundary" :disabled="busy || dataBusy" aria-label="外观"><div class="setting-row"><div><strong>主题</strong><p>选择你喜欢的星空底色</p></div><select v-model="settingsDraft.theme" aria-label="主题"><option v-for="option in themeOptions" :key="option.value" :value="option.value">{{ option.name }} · {{ option.description }}</option></select></div><div class="sticker-settings"><div class="sticker-settings-heading"><strong>贴纸小伙伴</strong><span>原创小图案，陪你收好灵感</span></div><div class="sticker-options" role="group" aria-label="贴纸风格"><button v-for="option in stickerOptions" :key="option.value" type="button" :class="['sticker-option', { chosen: settingsDraft.stickerPack === option.value }]" :aria-pressed="settingsDraft.stickerPack === option.value" :aria-label="option.name" :title="option.description" @click="settingsDraft.stickerPack = option.value"><StickerScene v-if="option.value !== 'none'" :pack="option.value" :theme="settingsDraft.theme === 'system' ? resolvedTheme : settingsDraft.theme" placement="preview" /><span v-else class="sticker-none" aria-hidden="true">—</span><strong>{{ option.name }}</strong><span>{{ option.description }}</span></button></div></div></fieldset>
            </section>
            <section v-show="settingsTab === 'habits'" id="settings-panel-habits" role="tabpanel" aria-labelledby="settings-tab-habits" class="settings-panel">
              <fieldset class="settings-section preferences-boundary" :disabled="busy || dataBusy" aria-label="使用习惯"><h3><Icon name="sliders" :size="16" />使用习惯</h3><div class="setting-row"><div><strong>流星启动画面</strong><p>仅首次打开短暂显示，可随时跳过</p></div><div class="startup-preference"><button type="button" class="startup-replay" :disabled="dataBusy" @click="replayStartup">重看一次</button><button type="button" :class="['switch', { on: settingsDraft.showStartup }]" role="switch" :aria-checked="settingsDraft.showStartup" aria-label="流星启动画面" :disabled="dataBusy" @click="settingsDraft.showStartup = !settingsDraft.showStartup"><span /></button></div></div><div class="setting-row"><div><strong>默认布局</strong><p>网格、列表，或横向快取的流星栏</p></div><select v-model="settingsDraft.layout" aria-label="默认布局"><option value="grid">卡片网格</option><option value="list">紧凑列表</option><option value="horizontal">流星栏 · 横向快取</option></select></div><div class="setting-row"><div><strong>窗口置顶</strong><p>{{ isDesktop ? '保持在其他应用窗口上方' : '桌面版生效；此处仅预览设置' }}</p></div><button type="button" :class="['switch', { on: settingsDraft.alwaysOnTop }]" role="switch" :aria-checked="settingsDraft.alwaysOnTop" aria-label="窗口置顶" @click="settingsDraft.alwaysOnTop = !settingsDraft.alwaysOnTop"><span /></button></div><div class="setting-row"><div><strong>呼出快捷键</strong><p>从任何应用打开星笺</p></div><input class="shortcut-input" v-model="settingsDraft.shortcut" aria-label="呼出快捷键" placeholder="CommandOrControl+Shift+V" /></div><div class="setting-row"><div><strong>分类 / 分组切换</strong><p>窗口内循环切换；输入文字和弹窗内不触发</p></div><kbd class="setting-shortcut">{{ modKey }} ⇧ ← / →</kbd></div></fieldset>
            </section>
            <section v-show="settingsTab === 'privacy'" id="settings-panel-privacy" role="tabpanel" aria-labelledby="settings-tab-privacy" class="settings-panel">
              <fieldset class="settings-section preferences-boundary" :disabled="busy || dataBusy" aria-label="记录与隐私"><h3><Icon name="shield" :size="16" />记录与隐私</h3><div class="setting-row"><div><strong>自动记录新复制的文本</strong><p>不捕获开启前的内容；随时可以暂停</p></div><button type="button" :class="['switch', { on: !settingsDraft.paused }]" role="switch" :aria-checked="!settingsDraft.paused" aria-label="自动记录新复制的文本" @click="settingsDraft.paused = !settingsDraft.paused"><span /></button></div><div class="setting-row"><div><strong>历史记录上限</strong><p>星标、永久保留与分组记录不计入上限</p></div><select v-model.number="settingsDraft.maxHistory" aria-label="历史记录上限"><option :value="0">不限条数</option><option :value="100">100 条</option><option :value="500">500 条</option><option :value="1000">1,000 条</option><option :value="5000">5,000 条</option></select></div><div class="setting-row"><div><strong>历史保留时长</strong><p>按最近更新时间计算；星标、永久保留与分组受保护</p></div><select v-model.number="settingsDraft.retentionDays" aria-label="历史保留时长"><option :value="0">不限天数</option><option :value="1">1 天</option><option :value="7">7 天</option><option :value="30">30 天</option><option :value="90">90 天</option><option :value="180">180 天</option><option :value="365">365 天</option></select></div><p class="retention-note">保存偏好后立即应用清理规则。桌面版还会在启动、新记录和约每分钟检查；暂停记录时也会按已选时长清理。浏览器演示只在保存偏好或新增内容时检查。永久保留、星标和已分组内容不会自动清理；重新捕获相同内容会更新保留起点。清理无法撤回，可先导出备份。同时选择「不限条数」和「不限天数」可关闭自动淘汰；重要内容也可单独标记永久保留。</p><div class="privacy-note"><Icon name="shield" :size="16" /><p>内容仅保存在本机，不上传云端。剪贴板可能含有私密内容，复制密码等敏感信息前请先暂停记录。</p></div></fieldset>
            </section>
            <section v-show="settingsTab === 'data'" id="settings-panel-data" role="tabpanel" aria-labelledby="settings-tab-data" class="settings-panel">
              <div class="settings-section backup-section"><h3><Icon name="folder" :size="16" />你的数据，由你保管</h3><fieldset class="data-settings-boundary" :disabled="busy"><DataSettings v-if="dataTabVisited" @busy="dataBusy = $event" /></fieldset><div class="backup-buttons"><button type="button" class="button subtle" @click="exportBackup" :disabled="busy || dataBusy"><Icon name="down" :size="16" />导出备份</button><button type="button" class="button subtle" @click="fileInput?.click()" :disabled="busy || dataBusy"><Icon name="upload" :size="16" />导入备份</button><button type="button" class="button subtle" :disabled="busy || dataBusy" @click="openCleanup">星尘整理</button></div><p class="field-note">导入、清理与目录操作会单独执行，不会替你保存其他 Tab 的修改。备份是可阅读的 JSON 文件，导入会合并记录。</p></div>
            </section>
            <section v-show="settingsTab === 'help'" id="settings-panel-help" role="tabpanel" aria-labelledby="settings-tab-help" class="settings-panel">
              <div class="settings-help-intro"><span class="settings-help-icon"><Icon name="spark" :size="24" /></span><div><h3>初见星笺</h3><p>四步认识记录、收藏、卡片和数据管理。随时重看，按自己的节奏熟悉。</p></div></div>
              <button ref="tutorialButton" type="button" class="button subtle tutorial-replay" :disabled="busy || dataBusy" @click="openGuide"><Icon name="play" :size="15" />重看新手教程</button>
              <p class="field-note">重看教程不会开启记录或改动设置，也会保留你在各个 Tab 中尚未保存的修改。</p>
              <div class="settings-help-shortcuts"><h3>常用快捷键</h3><dl>
                <div><dt>从其他应用呼出星笺</dt><dd><kbd>{{ shortcutLabel }}</kbd></dd></div>
                <div><dt>搜索记录</dt><dd><kbd>{{ modKey }} K</kbd></dd></div>
                <div><dt>切换分类与分组</dt><dd><kbd>{{ modKey }} ⇧ ← / →</kbd></dd></div>
                <div><dt>选择 / 复制记录</dt><dd><kbd>方向键 / Enter</kbd></dd></div>
                <div><dt>复制前九条记录</dt><dd><kbd>{{ modKey }} 1–9</kbd></dd></div>
                <div><dt>保存全部偏好</dt><dd><kbd>{{ modKey }} S</kbd></dd></div>
              </dl><p class="field-note">除全局呼出键外，请在星笺窗口中使用。输入文字、教程和弹窗中不会触发记录复制或分类切换。</p></div>
            </section>
          </div>
          <div class="modal-actions settings-actions">
            <div class="settings-save-state"><span role="status" aria-live="polite" :class="{ dirty: settingsDirty }">{{ settingsSaving ? '正在保存…' : settingsDirty ? '有未保存的修改' : '设置已同步' }}</span><small><kbd>{{ modKey }} S</kbd> 保存所有 Tab 的修改</small></div>
            <button type="button" class="button subtle" :disabled="busy || dataBusy" @click="closeModal">{{ settingsDirty ? '取消修改' : '完成' }}</button>
            <button type="submit" class="button primary" :disabled="busy || dataBusy || !settingsDirty">{{ settingsSaving ? '保存中…' : '保存全部偏好' }}<Icon name="check" :size="16" /></button>
          </div>
        </form>
        <CleanupPanel v-else-if="modal === 'clear'" @busy="dataBusy = $event" @changed="refresh" />
        <template v-else-if="modal === 'delete-group'"><p class="modal-intro">「{{ currentGroup?.name }}」中的片段仍保留在全部记录中，仅移除分组。</p><div class="modal-actions"><button class="button subtle" @click="closeModal">取消</button><button class="button danger" @click="deleteCurrentGroup" :disabled="busy || dataBusy">删除分组</button></div></template>
      </div>
    </dialog>
    <CodeStudio v-if="studioOpen" :clip="studioClip" :groups="state.groups" :initial-mode="studioClip && studioClip.kind !== 'code' ? 'handwriting' : 'code'" @close="studioOpen = false" @saved="refresh" />
    <input ref="fileInput" type="file" accept="application/json,.json" hidden @change="importBackup" />
    <Transition name="toast"><div v-if="toast" :class="['toast-message', { error: toastError }]" role="status"><Icon :name="toastError ? 'alert' : 'check'" :size="17" /><span>{{ toast }}</span><button class="icon-button" @click="toast = ''" aria-label="关闭提示"><Icon name="close" :size="14" /></button></div></Transition>
  </div>
</template>
