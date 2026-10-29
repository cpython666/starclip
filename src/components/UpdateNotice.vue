<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import {
  getUpdatePreferences,
  initialUpdatePreferences,
  initialUpdateStatus,
  onUpdatePreferencesChanged,
  skipUpdateVersion,
  updateDesktopAvailable,
  watchStarclipUpdates,
} from '../update-api'
import type { UpdatePreferences, UpdateStatus } from '../update-api'
import { t } from '../i18n'

const props = withDefaults(defineProps<{ blocked?: boolean }>(), { blocked: false })
const emit = defineEmits<{ 'open-settings': []; busy: [value: boolean] }>()

const status = ref<UpdateStatus>(initialUpdateStatus())
const preferences = ref<UpdatePreferences>(initialUpdatePreferences())
const preferencesLoaded = ref(false)
const statusLoaded = ref(false)
const skipBusy = ref(false)
const error = ref('')
let monitor: ReturnType<typeof watchStarclipUpdates> | undefined
let stopPreferences: (() => void) | undefined
let disposed = false
let statusRevision = -1
let preferenceRevision = -1

const candidate = computed(() => status.value.available)
const visible = computed(() => updateDesktopAvailable && !props.blocked && preferencesLoaded.value && statusLoaded.value
  && preferences.value.autoCheck && !!candidate.value
  && candidate.value.version !== status.value.currentVersion
  && candidate.value.version !== preferences.value.skippedVersion
  && ['available', 'ready'].includes(status.value.phase))
const ready = computed(() => status.value.phase === 'ready')

function acceptStatus(next: UpdateStatus) {
  if (disposed || next.revision < statusRevision) return
  statusRevision = next.revision
  status.value = next
  statusLoaded.value = true
}
function acceptPreferences(next: UpdatePreferences) {
  if (disposed || next.revision < preferenceRevision) return
  preferenceRevision = next.revision
  preferences.value = next
  preferencesLoaded.value = true
}
async function start() {
  if (disposed || !updateDesktopAvailable) return
  monitor = watchStarclipUpdates(acceptStatus)
  try {
    stopPreferences = await onUpdatePreferencesChanged(acceptPreferences)
    if (disposed) { stopPreferences?.(); return }
    await Promise.allSettled([getUpdatePreferences().then(acceptPreferences), monitor.read()])
  } catch {
    // Automatic checks are deliberately quiet. The help panel still exposes
    // the manual diagnostic and retry path.
  }
}
async function ignoreVersion() {
  const version = candidate.value?.version
  if (disposed || !visible.value || !version || skipBusy.value) return
  skipBusy.value = true
  error.value = ''
  try {
    acceptPreferences(await skipUpdateVersion(version))
  } catch {
    if (!disposed && candidate.value?.version === version) error.value = t('暂时无法隐藏提醒，请重试。')
  } finally {
    if (!disposed) skipBusy.value = false
  }
}
function openSettings() { if (!disposed && visible.value && !skipBusy.value) emit('open-settings') }
watch(skipBusy, value => emit('busy', value), { flush: 'sync' })
watch(() => candidate.value?.version, () => { error.value = '' })
const canRelease = computed(() => !skipBusy.value)
defineExpose({ canRelease })

onMounted(() => { void start() })
onBeforeUnmount(() => {
  disposed = true
  stopPreferences?.()
  stopPreferences = undefined
  monitor?.dispose()
  monitor = undefined
  emit('busy', false)
})
</script>

<template>
  <aside v-if="visible" class="update-notice" role="status" aria-live="polite">
    <span class="update-notice-dot" aria-hidden="true" />
    <span class="update-notice-copy">
      <strong>{{ ready ? t('新版 {version} 已准备就绪', { version: candidate?.version ?? '' }) : t('稳定版 {version} 可更新', { version: candidate?.version ?? '' }) }}</strong>
      <small>{{ ready ? t('更新包已下载并验证，打开帮助页安装并重启。') : t('稳定版更新已准备好，打开帮助页查看说明。') }}</small>
      <span v-if="error" class="update-notice-error" role="alert">{{ error }}</span>
    </span>
    <span class="update-notice-actions">
      <button type="button" class="button subtle" :disabled="skipBusy" @click="ignoreVersion">{{ skipBusy ? t('正在处理…') : t('忽略此版本') }}</button>
      <button type="button" class="button primary" :disabled="skipBusy" @click="openSettings">{{ t('查看更新') }}</button>
    </span>
  </aside>
</template>
