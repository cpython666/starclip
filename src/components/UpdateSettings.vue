<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue';
import { createRecoveryPoint, initialUpdateStatus, listRecoveryPoints, recoveryPendingPath, restartAfterUpdate, restoreRecoveryPoint, updateDesktopAvailable, watchStarclipUpdates } from '../update-api';
import type { RecoveryPoint } from '../update-api';
import { downloadProgress, fileSize, updateInProgress, updatePhaseText } from '../update-state';

const props = withDefaults(defineProps<{ disabled?: boolean; unsavedChanges?: boolean }>(), { disabled: false, unsavedChanges: false });
const emit = defineEmits<{ busy: [value: boolean]; 'restart-required': [] }>();
const status = ref(initialUpdateStatus());
const statusLoaded = ref(false);
const points = ref<RecoveryPoint[]>([]);
const pointsLoaded = ref(false);
const pendingPath = ref<string | null>(null);
const operation = ref<'load' | 'check' | 'download' | 'install' | 'snapshot' | 'restore' | 'restart' | null>(null);
const error = ref('');
const recoveryError = ref('');
const notice = ref('');
const installVersion = ref<string | null>(null);
const restoreSelected = ref<RecoveryPoint | null>(null);
const confirmation = ref<HTMLElement>();
const installDialog = ref<HTMLDialogElement>();
let disposed = false;
const monitor = watchStarclipUpdates(value => { status.value = value; statusLoaded.value = true; });
const needsRestart = computed(() => !!pendingPath.value || ['installed', 'restart-required'].includes(status.value.phase));
const working = computed(() => operation.value !== null || updateInProgress(status.value));
const blocked = computed(() => props.disabled || working.value || needsRestart.value);
const progress = computed(() => downloadProgress(status.value));
const canDownload = computed(() => !!status.value.available && ['available', 'error'].includes(status.value.phase));
const canInstall = computed(() => !!status.value.available && status.value.phase === 'ready');
watch(() => working.value || needsRestart.value, value => emit('busy', value), { immediate: true, flush: 'sync' });
watch(needsRestart, value => { if (value) emit('restart-required'); }, { flush: 'sync' });
watch([() => status.value.phase, () => status.value.available?.version], ([phase, version]) => {
  if (phase === 'ready' && version) {
    restoreSelected.value = null; installVersion.value = version;
  } else dismissInstall();
});
watch(installVersion, async version => {
  if (!version) return;
  await nextTick();
  if (!disposed && installVersion.value === version && !installDialog.value?.open) installDialog.value?.showModal();
});

function message(reason: unknown) { return reason instanceof Error ? reason.message : String(reason); }
function date(value: number | string) {
  if (typeof value === 'number' && value <= 0) return '时间未知';
  const parsed = new Date(value);
  return Number.isNaN(parsed.getTime()) ? '日期未提供' : parsed.toLocaleString('zh-CN', { hour12: false });
}
function reasonLabel(reason: string) {
  return ({ manual: '手动保存', 'before-update': '升级前', 'before-restore': '恢复前', 'before-migration': '数据升级前' } as Record<string, string>)[reason] || '恢复点';
}
async function perform(name: NonNullable<typeof operation.value>, action: () => Promise<void>) {
  if (disposed || !updateDesktopAvailable || props.disabled || working.value || (needsRestart.value && name !== 'restart')) return;
  operation.value = name; error.value = ''; notice.value = '';
  try { await action(); } catch (reason) { if (!disposed) error.value = message(reason); }
  finally { if (!disposed) operation.value = null; }
}
function refresh() {
  return perform('load', async () => {
    recoveryError.value = '';
    const results = await Promise.allSettled([monitor.read(), listRecoveryPoints(), recoveryPendingPath()]);
    if (disposed) return;
    const [update, recovery, pending] = results;
    if (update.status === 'rejected') error.value = `更新状态读取失败：${message(update.reason)}`;
    if (recovery.status === 'fulfilled') { points.value = recovery.value; pointsLoaded.value = true; }
    else recoveryError.value = `恢复点读取失败：${message(recovery.reason)}`;
    if (pending.status === 'fulfilled') pendingPath.value = pending.value;
    else recoveryError.value = [recoveryError.value, `数据位置读取失败：${message(pending.reason)}`].filter(Boolean).join('\n');
  });
}
function check() { return perform('check', () => monitor.check()); }
function download() {
  const version = status.value.available?.version;
  if (!version || !canDownload.value) return;
  return perform('download', async () => {
    try { await monitor.download(version); }
    catch (reason) {
      try { await monitor.read(); } catch { /* Preserve the download error. */ }
      throw reason;
    }
  });
}
function confirmInstall() {
  if (blocked.value || !canInstall.value) return;
  restoreSelected.value = null; installVersion.value = status.value.available!.version;
}
async function confirmRestore(point: RecoveryPoint) {
  if (blocked.value || !point.compatible) return;
  dismissInstall(); restoreSelected.value = point;
  await nextTick(); if (!disposed) confirmation.value?.focus();
}
function cancelConfirmation() {
  if (blocked.value) return;
  restoreSelected.value = null;
}
function dismissInstall() {
  installDialog.value?.close();
  installVersion.value = null;
}
function install() {
  const approvedVersion = installVersion.value;
  if (!approvedVersion || props.unsavedChanges || !canInstall.value || status.value.available?.version !== approvedVersion) return;
  return perform('install', async () => {
    dismissInstall();
    try { await monitor.install(approvedVersion); }
    catch (reason) {
      try { await monitor.read(); } catch { /* Preserve the installation error. */ }
      if (status.value.phase !== 'installed') throw reason;
    }
    if (!disposed && status.value.phase === 'installed') await restartInstalled();
  });
}
function createSnapshot() {
  if (props.unsavedChanges) return;
  return perform('snapshot', async () => {
    const point = await createRecoveryPoint();
    if (disposed) return;
    points.value = [point, ...points.value.filter(item => item.id !== point.id)]; pointsLoaded.value = true;
    notice.value = '恢复点已保存在本机。';
  });
}
function restore() {
  const selected = restoreSelected.value;
  if (!selected?.compatible || props.unsavedChanges) return;
  return perform('restore', async () => {
    try {
      await restoreRecoveryPoint(selected.id);
      const pending = await recoveryPendingPath();
      if (disposed) return;
      pendingPath.value = pending; restoreSelected.value = null;
      if (!pending) throw new Error('未能确认恢复后的数据位置，请刷新状态后重试。');
      notice.value = '恢复数据已准备好，重启星笺后生效。';
    } catch (reason) {
      // A lost response must not unlock writes after the backend has switched directories.
      try {
        const pending = await recoveryPendingPath();
        if (!disposed && pending) { pendingPath.value = pending; restoreSelected.value = null; }
      } catch { /* Preserve the restore error; the backend continues to guard writes. */ }
      throw reason;
    }
  });
}
async function restartInstalled() {
  notice.value = '正在重启星笺…';
  try { await restartAfterUpdate(); }
  catch (reason) { throw new Error(`自动重启未完成，请再次点击「重启星笺」。${message(reason)}`); }
}
function restart() { return perform('restart', restartInstalled); }
defineExpose({ refresh });
onMounted(() => { if (updateDesktopAvailable) void refresh(); });
onBeforeUnmount(() => { disposed = true; installDialog.value?.close(); monitor.dispose(); emit('busy', false); });
</script>

<template>
  <section class="update-settings" aria-labelledby="software-update-heading" :aria-busy="working">
    <div class="update-heading"><h3 id="software-update-heading">软件更新与恢复</h3><span class="update-version">当前版本 {{ status.currentVersion }}</span></div>
    <div v-if="!updateDesktopAvailable" class="update-card">
      <strong>请在星笺桌面版检查更新</strong>
      <p>浏览器演示没有已安装的软件和本机恢复点，不会检查、下载或安装桌面更新。</p>
    </div>
    <template v-else>
      <p v-if="!statusLoaded" class="update-hint" role="status">{{ operation === 'load' ? '正在读取本机状态…' : '尚未读取更新状态。' }}</p>
      <p v-else class="update-hint" :class="{ 'update-error': ['error', 'restart-required'].includes(status.phase) }" :role="['error', 'restart-required'].includes(status.phase) ? 'alert' : 'status'">{{ updatePhaseText(status) }}</p>
      <p class="update-hint">手动检查或下载时才会连接 GitHub 更新服务。下载完成后可稍后安装；确认安装时先保存恢复点，再安装并重启，历史、偏好和星伴成长保留在本机。</p>
      <div v-if="status.available" class="update-card update-release">
        <div class="update-release-heading"><strong>新版本 {{ status.available.version }}</strong><span v-if="status.available.date">{{ date(status.available.date) }}</span></div>
        <p v-if="status.available.notes" class="update-notes">{{ status.available.notes }}</p>
        <p v-else>此版本未提供更新说明。</p>
      </div>
      <div v-if="status.phase === 'downloading'" class="update-download" role="status">
        <progress aria-label="更新下载进度" :value="progress" max="100"></progress>
        <span>{{ fileSize(status.downloaded) }}<template v-if="status.total"> / {{ fileSize(status.total) }}</template><template v-if="progress !== undefined"> · {{ Math.floor(progress) }}%</template></span>
      </div>
      <div v-if="!needsRestart" class="update-actions">
        <button type="button" class="update-button" :disabled="blocked || (statusLoaded && !status.configured)" @click="statusLoaded ? check() : refresh()">{{ operation === 'check' || status.phase === 'checking' ? '正在检查…' : statusLoaded ? '检查更新' : '重新读取状态' }}</button>
        <button v-if="canDownload" type="button" class="update-button update-primary" :disabled="blocked" @click="download">{{ status.phase === 'error' ? '重新下载更新' : '下载更新' }}</button>
        <button v-if="canInstall" type="button" class="update-button update-primary" :disabled="blocked" @click="confirmInstall">安装并重启</button>
      </div>
      <div v-if="needsRestart" class="update-confirmation" role="status">
        <strong>{{ pendingPath ? '恢复数据已准备好，重启后继续' : status.phase === 'installed' ? '新版已安装，重启后继续' : '安装未完成，请先重启星笺' }}</strong>
        <p>当前编辑已暂停，请重启星笺后继续使用。</p>
        <details v-if="pendingPath"><summary>下次启动的数据位置</summary><code>{{ pendingPath }}</code></details>
        <button type="button" class="update-button update-primary" :disabled="operation !== null || disabled" @click="restart">{{ operation === 'restart' ? '正在重启…' : '重启星笺' }}</button>
      </div>
      <dialog v-if="installVersion && !needsRestart" ref="installDialog" class="update-ready-dialog" aria-labelledby="update-ready-heading" aria-describedby="update-ready-description" @cancel.prevent.stop="dismissInstall" @close.stop @keydown.stop @click="event => { if (event.target === installDialog) dismissInstall() }">
        <div class="update-ready-content">
          <div class="update-ready-heading"><h3 id="update-ready-heading">新版本 {{ installVersion }} 已准备就绪</h3><button type="button" class="update-ready-close" aria-label="稍后安装更新" @click="dismissInstall">×</button></div>
          <p id="update-ready-description">更新包已下载并通过验证。安装前会保存当前数据的恢复点，随后暂时关闭星笺，完成后自动打开。</p>
          <p v-if="unsavedChanges" class="update-warning">有尚未保存的偏好，请先选择「稍后再说」并保存，再安装更新。</p>
          <div class="update-actions update-confirm-actions"><button type="button" class="update-button" autofocus @click="dismissInstall">稍后再说</button><button type="button" class="update-button update-primary" :disabled="blocked || unsavedChanges" @click="install">安装并重启</button></div>
        </div>
      </dialog>
      <div class="update-recovery">
        <div class="update-heading"><h4>恢复点</h4><button type="button" class="update-refresh" :disabled="blocked" @click="refresh">刷新状态</button></div>
        <p class="update-hint">恢复点保存历史、分组、已保存的偏好与星伴成长。恢复会在新目录中使用旧数据，当前目录保留；不会合并之后新增的记录。</p>
        <button type="button" class="update-button" :disabled="blocked || unsavedChanges" @click="createSnapshot">{{ operation === 'snapshot' ? '正在保存…' : '保存当前恢复点' }}</button>
        <p v-if="unsavedChanges" class="update-hint update-warning">请先点击下方「保存全部偏好」，再安装更新或创建、恢复数据。</p>
        <p v-if="pointsLoaded && !points.length" class="update-hint">还没有恢复点。可以手动保存，安装更新前也会自动保存。</p>
        <ul v-if="points.length" class="recovery-list" aria-label="本机恢复点">
          <li v-for="point in points" :key="point.id" class="update-card">
            <div class="recovery-row"><div><strong>{{ date(point.createdAt) }}</strong><span>{{ reasonLabel(point.reason) }} · v{{ point.appVersion }} · {{ fileSize(point.totalBytes) }}</span></div><button type="button" class="update-button" :disabled="blocked || unsavedChanges || !point.compatible" @click="confirmRestore(point)">恢复此时数据</button></div>
            <p v-if="!point.compatible" class="update-warning">此恢复点暂不可用，可能是数据损坏或版本不兼容。请查看下方原因；若由较新版本创建，请先升级星笺。</p>
            <details><summary>查看保存位置</summary><code>{{ point.path }}</code></details>
            <ul v-if="point.warnings.length" class="update-warnings"><li v-for="warning in point.warnings" :key="warning">{{ warning }}</li></ul>
          </li>
        </ul>
        <div v-if="restoreSelected && !needsRestart" ref="confirmation" class="update-confirmation" tabindex="-1" role="region" aria-label="确认恢复旧数据">
          <strong>恢复到 {{ date(restoreSelected.createdAt) }}？</strong>
          <p>将使用这份恢复点中的<strong>旧历史、旧偏好与旧星伴成长</strong>，之后新增或修改的内容不会合并。星笺会先保存当前数据的恢复点，并保留当前目录。确认后需重启。</p>
          <div class="update-actions update-confirm-actions"><button type="button" class="update-button" :disabled="blocked" @click="cancelConfirmation">取消</button><button type="button" class="update-button update-primary" :disabled="blocked || unsavedChanges" @click="restore">确认恢复旧数据</button></div>
        </div>
        <p v-if="recoveryError" class="update-error" role="alert">{{ recoveryError }}</p>
      </div>
      <p v-if="error" class="update-error" role="alert">{{ error }}</p>
      <p v-else-if="notice" class="update-notice" role="status">{{ notice }}</p>
      <p class="update-privacy">恢复点以明文保存在本机，不上传。重要数据也建议另存到其他磁盘。</p>
    </template>
  </section>
</template>

<style scoped>
.update-settings{min-width:0;padding:16px 0;border-top:1px solid var(--line);color:var(--text)}
.update-ready-dialog{width:min(440px,calc(100vw - 40px));max-height:calc(100dvh - 40px);padding:0;border:1px solid var(--line);border-radius:18px;background:var(--panel);color:var(--text);box-shadow:0 24px 80px #0005;overflow:auto}.update-ready-dialog::backdrop{background:#080b1866;backdrop-filter:blur(3px)}.update-ready-content{padding:26px}.update-ready-heading{display:flex;align-items:flex-start;justify-content:space-between;gap:16px}.update-ready-heading h3{margin:0;font-size:17px;line-height:1.6;font-weight:500;overflow-wrap:anywhere}.update-ready-close{flex-shrink:0;display:grid;place-items:center;width:28px;height:28px;border:0;border-radius:6px;color:var(--muted);background:transparent;font-size:25px;cursor:pointer}.update-ready-close:hover{background:var(--input);color:var(--text)}.update-ready-content>p{margin:16px 0;color:var(--muted);font-size:12px;line-height:1.9}.update-ready-content>.update-warning{color:var(--gold)}.update-ready-dialog .update-actions{margin-top:24px}.update-ready-dialog .update-button{font-size:12px;min-height:40px;padding:9px 16px}.update-ready-dialog .update-primary{flex:1}.update-ready-dialog :focus-visible{outline:2px solid var(--gold);outline-offset:3px}
.update-heading,.update-release-heading,.recovery-row{display:flex;align-items:center;justify-content:space-between;gap:12px}.update-heading{margin-bottom:10px}.update-heading h3,.update-heading h4{margin:0;font-size:11px;font-weight:500;color:var(--gold)}.update-version,.update-release-heading>span{font-size:10px;color:var(--muted);font-variant-numeric:tabular-nums}.update-hint,.update-card p,.update-confirmation p{font-size:10px;line-height:1.8;color:var(--muted);margin:9px 0}.update-card,.update-confirmation{padding:12px;border:1px solid var(--line);border-radius:8px;background:var(--input);margin-top:12px;min-width:0}.update-card strong,.update-confirmation>strong{font-size:11px;font-weight:500}.update-card p:last-child{margin-bottom:0}.update-notes{white-space:pre-wrap;overflow-wrap:anywhere;max-height:200px;overflow-y:auto;user-select:text}.update-actions{display:flex;flex-wrap:wrap;gap:8px;margin-top:12px}.update-button{display:inline-flex;align-items:center;justify-content:center;font-size:10px;line-height:1.5;padding:8px 10px;border:1px solid var(--line);border-radius:6px;background:var(--panel);color:var(--text);cursor:pointer}.update-button:hover{border-color:var(--muted);background:var(--input)}.update-primary{color:var(--gold);border-color:var(--gold)}.update-settings button:disabled{opacity:.45;cursor:default}.update-refresh{font-size:10px;color:var(--muted);padding:5px;border-radius:4px}.update-refresh:hover{background:var(--input);color:var(--text)}.update-confirmation{border-color:var(--gold);background:var(--panel)}.update-confirmation>strong{display:block;color:var(--gold)}.update-confirmation p strong{font-weight:500;color:var(--text)}.update-confirm-actions{justify-content:flex-end}.update-recovery{margin-top:22px;padding-top:16px;border-top:1px solid var(--line)}.recovery-list{margin:12px 0 0;padding:0;list-style:none}.recovery-list>.update-card{margin-top:8px}.recovery-row>div{display:flex;flex-direction:column;gap:7px;min-width:0}.recovery-row>div>span{font-size:9px;color:var(--muted);line-height:1.5}.recovery-row .update-button{flex-shrink:0}.update-settings details{font-size:10px;line-height:1.8;color:var(--muted);margin-top:9px}.update-settings summary{cursor:pointer}.update-settings code{display:block;font:10px/1.8 ui-monospace,SFMono-Regular,Menlo,Consolas,monospace;white-space:pre-wrap;overflow-wrap:anywhere;user-select:text;margin-top:6px}.update-warning,.update-settings .update-warning{color:var(--gold)}.update-warnings{font-size:10px;color:var(--gold);line-height:1.8;padding-left:18px}.update-error,.update-settings .update-error,.update-notice{font-size:10px;line-height:1.8;white-space:pre-wrap;overflow-wrap:anywhere;margin-top:12px}.update-error,.update-settings .update-error{color:var(--danger)}.update-notice{color:var(--mint)}.update-privacy{font-size:9px;color:var(--faint);line-height:1.8;margin:12px 0 0}.update-download{display:flex;flex-direction:column;gap:7px;margin-top:12px}.update-download progress{width:100%;height:7px;accent-color:var(--gold)}.update-download span{font-size:10px;color:var(--muted);font-variant-numeric:tabular-nums}.update-settings :focus-visible{outline:2px solid var(--gold);outline-offset:3px}
@media(max-width:600px){.update-heading,.update-release-heading,.recovery-row{align-items:flex-start;flex-wrap:wrap}.update-actions .update-button{flex:1}.recovery-row .update-button{width:100%}}
</style>
