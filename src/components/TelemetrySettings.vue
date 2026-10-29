<script setup lang="ts">
import { computed, onMounted, onBeforeUnmount, ref, watch } from 'vue';
import { errorText, t } from '../i18n';
import { clearTelemetryQueue, flushTelemetry, getTelemetryState, initialTelemetryState, onTelemetryStateChanged, setTelemetryEnabled, telemetryDesktopAvailable, usageEventNames } from '../telemetry-api';
import type { TelemetryState, UsageEvent } from '../telemetry-api';

const props = defineProps<{ disabled?: boolean }>();
const emit = defineEmits<{ busy: [value: boolean] }>();
const state = ref(initialTelemetryState());
const loaded = ref(!telemetryDesktopAvailable);
const working = ref(false);
const error = ref('');
const retryChoice = ref<boolean | null>(null);
let disposed = false;
let stop: (() => void) | undefined;
const canRelease = computed(() => loaded.value && !working.value && retryChoice.value === null);
defineExpose({ canRelease });
watch(working, value => emit('busy', value), { flush: 'sync' });
const labels: Record<UsageEvent, string> = {
  app_started: '启动应用', studio_opened: '打开星绘坊', card_exported: '导出图片卡片',
  backup_exported: '导出备份', backup_imported: '导入备份', update_check_completed: '完成更新检查', font_downloaded: '下载字体',
  update_download_completed: '更新包下载完成', update_install_started: '开始安装更新',
};
// Render only the known schema even if an unexpected native payload is received.
const pending = computed(() => usageEventNames.flatMap(name => {
  const count = state.value.queuedEvents.find(event => event.name === name)?.count;
  return Number.isInteger(count) && count! > 0 && count! <= 100 ? [{ name, count: count! }] : [];
}));
const result = computed(() => ({
  never: '', sent: '', failed: '',
  disabled: '统计已关闭，待发送事件已清空。', unconfigured: '接收端未配置，当前不会收集或发送使用统计。',
}[state.value.lastResult]));
function accept(next: TelemetryState) {
  if (!disposed && next.revision >= state.value.revision) state.value = next;
}
async function read() {
  if (!telemetryDesktopAvailable || disposed || working.value) return;
  working.value = true; error.value = '';
  try {
    if (!stop) {
      const unsubscribe = await onTelemetryStateChanged(accept);
      if (disposed) { unsubscribe(); return; }
      stop = unsubscribe;
    }
    accept(await getTelemetryState());
    if (!disposed) loaded.value = true;
  } catch (e) { if (!disposed) error.value = errorText(e); }
  finally { working.value = false; }
}
async function perform(action: () => Promise<TelemetryState>, silent = false) {
  if (!loaded.value || props.disabled || working.value || !telemetryDesktopAvailable) return;
  working.value = true; error.value = '';
  try { accept(await action()); } catch (e) { if (!disposed && !silent) error.value = errorText(e); }
  finally { working.value = false; }
}
function toggle() {
  if (retryChoice.value !== null) return;
  if (!state.value.enabled && !state.value.configured) return;
  return saveChoice(!state.value.enabled);
}
function saveChoice(enabled: boolean) {
  return perform(async () => {
    try { const next = await setTelemetryEnabled(enabled); retryChoice.value = null; return next; }
    catch (e) { retryChoice.value = enabled; throw e; }
  });
}
function send() {
  if (!state.value.enabled || !state.value.configured || !pending.value.length) return;
  return perform(flushTelemetry, true);
}
onMounted(read);
onBeforeUnmount(() => { disposed = true; stop?.(); emit('busy', false); });
</script>

<template>
  <section class="telemetry-settings" :aria-label="t('可选使用统计')">
    <h3>{{ t('可选使用统计') }}</h3>
    <div class="setting-row">
      <div><strong>{{ t('帮助改进星笺') }}</strong><p>{{ t('默认开启 · 随时关闭 · 独立即时保存') }}</p></div>
      <button type="button" role="switch" :class="['switch', { on: state.enabled }]" :aria-checked="state.enabled" :aria-label="t('帮助改进星笺')" :disabled="!loaded || disabled || working || retryChoice !== null || !telemetryDesktopAvailable || (!state.configured && !state.enabled)" @click="toggle"><span /></button>
    </div>
    <p>{{ t('固定功能的使用次数发送给 Google Analytics 4；可随时关闭。') }}</p>
    <p>{{ t('使用统计用于改进功能与更新体验。只发送固定事件及次数、应用版本、系统与主版本、应用架构、机型代号或产品系列，以及每次运行更换的随机编号。') }}</p>
    <p>{{ t('不收集序列号、电脑名称、用户名或永久设备标识。机型无法安全识别时只记录通用类别。') }}</p>
    <p>{{ t('不收集剪贴内容、搜索词、图片、标题、链接、文件路径，也不录屏。接收端会接触网络 IP，但本实现不记录或转发它；云服务仍按各自政策处理请求。') }}</p>
    <p v-if="!telemetryDesktopAvailable" class="telemetry-status">{{ t('使用统计设置仅在桌面版可用。') }}</p>
    <p v-else-if="loaded && !state.configured" class="telemetry-status">{{ t('接收端未配置，当前不会收集或发送使用统计。') }}</p>
    <p v-else-if="state.endpointHost">{{ t('接收端：{host}', { host: state.endpointHost }) }}</p>
    <details>
      <summary>{{ t('查看统计范围与待发送事件') }}</summary>
      <p>{{ t('统计范围：启动应用、打开星绘坊、导出图片卡片、导入与导出备份、完成更新检查、更新包下载完成、开始安装更新、下载字体。') }}</p>
      <p>{{ t('统计在后台静默发送，失败不提示。断网时仅缓存固定次数，最多保留 72 小时、每种 100 次；自动逐步延长重试间隔，最长一小时。关闭统计会清空待发计数和本地缓存。') }}</p>
      <p>{{ t('缓存不含剪贴内容、设备信息或会话编号；升级版本时舍弃旧缓存。补报按收到时间统计，可能漏计或重复，不用于精确追踪个人。') }}</p>
      <p>{{ t('安装计数表示开始尝试，不代表安装成功；退出前未送达的计数可能丢失。') }}</p>
      <ul v-if="pending.length"><li v-for="event in pending" :key="event.name">{{ t(labels[event.name]) }} · {{ event.count }}</li></ul>
      <p v-else>{{ t('没有待发送事件') }}</p>
      <div class="telemetry-actions">
        <button type="button" class="button subtle" :disabled="disabled || working || !state.enabled || !state.configured || !pending.length" @click="send">{{ t('发送待发统计') }}</button>
        <button type="button" class="button subtle" :disabled="disabled || working || !pending.length" @click="perform(clearTelemetryQueue)">{{ t('清空待发送事件') }}</button>
      </div>
      <p v-if="result" role="status">{{ t(result) }}</p>
    </details>
    <p v-if="error" role="alert" class="telemetry-error">{{ error }}</p>
    <button v-if="retryChoice !== null" type="button" class="button subtle" :disabled="disabled || working" @click="saveChoice(retryChoice)">{{ t('重试保存统计选择') }}</button>
    <button v-if="telemetryDesktopAvailable && !loaded" type="button" class="button subtle" :disabled="working" @click="read">{{ t('重新读取') }}</button>
  </section>
</template>

<style scoped>
.telemetry-settings{border-top:1px solid var(--line);padding-top:24px;margin-top:28px;min-width:0}.telemetry-settings h3{font-size:13px;color:var(--gold);margin:0 0 18px}.telemetry-settings p,.telemetry-settings li{font-size:12px;line-height:1.8;color:var(--muted);overflow-wrap:anywhere}.telemetry-settings summary{font-size:12px;cursor:pointer;color:var(--text);padding:8px 0}.telemetry-status{padding:12px;background:var(--input);border:1px solid var(--line);border-radius:9px}.telemetry-actions{display:flex;gap:10px;flex-wrap:wrap}.telemetry-settings .telemetry-error{color:var(--danger)}
</style>
