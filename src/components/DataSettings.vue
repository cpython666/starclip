<script setup lang="ts">
import { computed, nextTick, onMounted, ref, watch } from 'vue';
import { api, isDesktop } from '../api';
import type { DataDirectoryMode, DataDirectoryPlan, DataLocation } from '../types';
import Icon from './Icon.vue';

const emit = defineEmits<{ busy: [value: boolean] }>();
const location = ref<DataLocation | null>(null);
const plan = ref<DataDirectoryPlan | null>(null);
const operation = ref<'load' | 'choose' | 'open' | 'apply' | 'restart' | null>(null);
const error = ref('');
const notice = ref('');
const confirmation = ref<HTMLElement>();
const pending = computed(() => location.value?.pendingPath || null);
const blocked = computed(() => operation.value !== null || pending.value !== null);
const warnings = computed(() => [...new Set([...(location.value?.warnings || []), ...(plan.value?.warnings || [])])]);
watch(blocked, value => emit('busy', value), { immediate: true, flush: 'sync' });

function message(reason: unknown) { return reason instanceof Error ? reason.message : String(reason); }
function bytes(value: number) {
  if (!Number.isFinite(value) || value < 0) return '暂不可用';
  if (value < 1024) return `${Math.round(value)} B`;
  const units = ['KiB', 'MiB', 'GiB', 'TiB'];
  let amount = value / 1024, unit = 0;
  while (amount >= 1024 && unit < units.length - 1) { amount /= 1024; unit++; }
  return `${amount.toLocaleString('zh-CN', { maximumFractionDigits: 2 })} ${units[unit]}`;
}
async function perform(name: NonNullable<typeof operation.value>, action: () => Promise<void>) {
  if (!isDesktop || operation.value || (pending.value && name !== 'restart')) return;
  operation.value = name; error.value = ''; notice.value = '';
  try { await action(); } catch (reason) { error.value = message(reason); } finally { operation.value = null; }
}
function refresh() {
  return perform('load', async () => { location.value = await api.getDataLocation(); });
}
defineExpose({ refresh });
function openFolder() {
  return perform('open', async () => { await api.openDataDirectory(); });
}
function choose(mode: DataDirectoryMode) {
  return perform('choose', async () => {
    plan.value = null;
    const selected = await api.chooseDataDirectory(mode);
    if (!selected) { notice.value = '已取消选择，数据位置未更改。'; return; }
    plan.value = selected;
    await nextTick(); confirmation.value?.focus();
  });
}
function cancel() {
  if (blocked.value) return;
  plan.value = null; error.value = ''; notice.value = '已取消，数据位置未更改。';
}
function restart() {
  return perform('restart', async () => {
    notice.value = '正在重启星笺…';
    try { await api.restartApp(); }
    catch (reason) { throw new Error(`自动重启未完成，请点击「重启星笺」重试。${message(reason)}`); }
  });
}
function apply() {
  const approved = plan.value;
  if (!approved) return;
  return perform('apply', async () => {
    try {
      location.value = await api.applyDataDirectory(approved.mode, approved.targetPath);
    } catch (reason) {
      // A failed response must not let the UI continue writing if the backend already switched.
      try { location.value = await api.getDataLocation(); } catch { /* Keep the original actionable error. */ }
      if (pending.value) plan.value = null;
      throw reason;
    }
    plan.value = null;
    notice.value = '数据位置已切换，正在重启星笺…';
    operation.value = 'restart';
    try { await api.restartApp(); }
    catch (reason) { throw new Error(`数据位置已切换，但自动重启未完成。请点击「重启星笺」继续。${message(reason)}`); }
  });
}
onMounted(() => { if (isDesktop) void refresh(); });
</script>

<template>
  <section class="data-settings" aria-labelledby="data-location-heading" :aria-busy="operation !== null">
    <div class="data-heading">
      <h3 id="data-location-heading"><Icon name="folder" :size="16" />数据位置</h3>
      <button v-if="isDesktop" type="button" class="data-refresh" :disabled="blocked" @click="refresh">{{ operation === 'load' ? '读取中…' : '刷新位置' }}</button>
    </div>
    <div v-if="!isDesktop" class="data-demo">
      <strong>数据位置管理需要桌面版</strong>
      <p>浏览器演示使用内存中的示例，不持久保存，也没有可迁移的本机数据目录。请在桌面版查看位置、打开文件夹或迁移数据。</p>
    </div>
    <template v-else>
      <p v-if="operation === 'load' && !location" class="data-hint" role="status">正在读取当前数据位置与占用…</p>
      <p v-else-if="!location" class="data-hint">暂时无法读取当前位置，请查看下方原因后重试。</p>
      <template v-if="location">
        <div class="data-current">
          <span class="data-label">当前数据目录</span>
          <code class="data-path">{{ location.currentPath }}</code>
          <dl class="data-sizes">
            <div><dt>受管数据</dt><dd>{{ bytes(location.totalBytes) }}</dd></div>
            <div><dt>其中数据库文件</dt><dd>{{ bytes(location.databaseBytes) }}</dd></div>
          </dl>
          <p class="data-size-note">包含数据库及日志、月光笺配置和目录内受管渲染器；不包含系统 WebView 缓存或其他无关文件。</p>
        </div>
        <details class="data-details">
          <summary>默认位置与目录配置</summary>
          <span class="data-label">默认数据目录</span><code class="data-path">{{ location.defaultPath }}</code>
          <span class="data-label">位置配置文件</span><code class="data-path">{{ location.configPath }}</code>
        </details>
        <div v-if="pending" class="data-pending" role="status">
          <strong>位置已切换，重启后继续使用</strong>
          <span class="data-label">下次启动使用</span><code class="data-path">{{ pending }}</code>
          <p>当前写入已暂停，旧目录保留。请重启星笺后继续编辑或保存。</p>
          <button type="button" class="data-button data-primary" :disabled="operation !== null" @click="restart">{{ operation === 'restart' ? '正在重启…' : '重启星笺' }}</button>
        </div>
        <template v-else>
          <div class="data-actions">
            <button type="button" class="data-button" :disabled="blocked" @click="openFolder"><Icon name="folder" :size="14" />打开文件夹</button>
            <button type="button" class="data-button" :disabled="blocked" @click="choose('move')">选择迁移目标</button>
            <button type="button" class="data-button" :disabled="blocked" @click="choose('use')">使用已有数据目录</button>
          </div>
          <p class="data-hint">迁移会在所选位置新建专用目录，不覆盖已有目录。包括历史、星标、分组、已保存的偏好和目录内手写组件；旧目录保留。目录外的手写组件不会复制。</p>
          <p class="data-hint">确认后将立即重启，请先保存其他偏好。迁移与 JSON 备份不同：JSON 备份只包含片段和分组。</p>
        </template>
      </template>
      <div v-if="plan && !pending" ref="confirmation" class="data-confirmation" tabindex="-1" role="region" aria-label="确认数据目录变更">
        <h4>{{ plan.mode === 'move' ? '确认迁移到新位置' : '确认使用已有数据' }}</h4>
        <span class="data-label">{{ plan.mode === 'move' ? '保留的原目录' : '即将停止使用的目录（保留）' }}</span>
        <code class="data-path">{{ plan.currentPath }}</code>
        <span class="data-label">{{ plan.mode === 'move' ? '将新建的数据目录' : '将改用的数据目录' }}</span>
        <code class="data-path">{{ plan.targetPath }}</code>
        <p class="data-plan-size">{{ plan.mode === 'move' ? '预计迁移受管数据' : '目标受管数据' }}：{{ bytes(plan.totalBytes) }}</p>
        <p v-if="plan.mode === 'move'">确认后复制上述受管数据并切换位置，随后重启。原目录保留，尚未保存的偏好不会自动保存。</p>
        <p v-else>确认后使用目标目录中的历史、分组与偏好，随后重启。<strong>不会合并当前记录</strong>，当前目录保留，尚未保存的偏好不会自动保存。</p>
        <ul v-if="warnings.length" class="data-warnings"><li v-for="warning in warnings" :key="warning">{{ warning }}</li></ul>
        <div class="data-confirm-actions">
          <button type="button" class="data-button" :disabled="blocked" @click="cancel">取消</button>
          <button type="button" class="data-button data-primary" :disabled="blocked" @click="apply">{{ operation === 'apply' ? '正在处理…' : plan.mode === 'move' ? '确认迁移并重启' : '确认切换并重启' }}</button>
        </div>
      </div>
      <ul v-if="warnings.length && !plan" class="data-warnings"><li v-for="warning in warnings" :key="warning">{{ warning }}</li></ul>
      <p v-if="error" class="data-error" role="alert">{{ error }}</p>
      <p v-else-if="notice" class="data-notice" role="status">{{ notice }}</p>
      <p class="data-privacy">数据以明文保存在本机所选目录，星笺不会上传到云端。</p>
    </template>
  </section>
</template>

<style scoped>
.data-settings{margin-top:18px;padding-top:18px;border-top:1px solid var(--line);min-width:0;color:var(--text)}
.data-heading{display:flex;align-items:center;justify-content:space-between;gap:12px;margin-bottom:12px}.data-heading h3{display:flex;align-items:center;gap:8px;margin:0;color:var(--gold);font-size:11px;font-weight:500}.data-refresh{font-size:10px;color:var(--muted);padding:5px;border-radius:4px}.data-refresh:hover{color:var(--text);background:var(--input)}
.data-current,.data-demo{border:1px solid var(--line);background:var(--input);border-radius:9px;padding:13px}.data-label{display:block;font-size:9px;color:var(--muted);margin-bottom:6px}.data-path{display:block;white-space:pre-wrap;overflow-wrap:anywhere;word-break:break-word;font:11px/1.7 ui-monospace,SFMono-Regular,Menlo,Consolas,monospace;color:var(--text);user-select:text}.data-sizes{display:flex;gap:25px;flex-wrap:wrap;margin:13px 0 0}.data-sizes>div{display:flex;gap:9px;align-items:baseline}.data-sizes dt{font-size:9px;color:var(--muted)}.data-sizes dd{font-size:11px;margin:0;font-variant-numeric:tabular-nums}.data-size-note{font-size:9px;line-height:1.7;color:var(--muted);margin-top:10px}
.data-details{margin:10px 0;font-size:10px;color:var(--muted)}.data-details summary{cursor:pointer;padding:4px 0}.data-details .data-label{margin-top:10px}.data-details .data-path{font-size:10px;color:var(--muted)}.data-actions,.data-confirm-actions{display:flex;gap:8px;flex-wrap:wrap;margin-top:12px}.data-button{display:inline-flex;justify-content:center;align-items:center;gap:6px;font-size:10px;line-height:1.5;border:1px solid var(--line);border-radius:6px;padding:8px 10px;background:var(--panel);color:var(--text);cursor:pointer}.data-button:hover{background:var(--input);border-color:var(--muted)}.data-primary{color:var(--gold);border-color:var(--gold);background:var(--input)}.data-settings button:disabled{opacity:.45;cursor:default}
.data-hint,.data-privacy,.data-demo p{font-size:10px;line-height:1.8;color:var(--muted);margin:10px 0 0}.data-demo strong{font-size:11px;font-weight:500;color:var(--text)}.data-privacy{color:var(--faint);font-size:9px}.data-confirmation,.data-pending{padding:14px;margin-top:15px;border:1px solid var(--gold);border-radius:9px;background:var(--panel)}.data-confirmation h4,.data-pending>strong{font-size:12px;color:var(--gold);font-weight:500;margin:0 0 12px;display:block}.data-confirmation .data-label:not(:first-child),.data-pending .data-label{margin-top:12px}.data-confirmation p,.data-pending p{font-size:10px;line-height:1.8;color:var(--muted);margin-top:11px}.data-confirmation p strong{font-weight:500;color:var(--text)}.data-plan-size{font-variant-numeric:tabular-nums}.data-confirm-actions{justify-content:flex-end;margin-top:14px}.data-pending .data-button{margin-top:12px}.data-warnings{color:var(--gold);font-size:10px;line-height:1.8;margin:12px 0 0;padding-left:17px;overflow-wrap:anywhere}.data-error,.data-notice{font-size:10px;line-height:1.8;margin-top:12px;white-space:pre-wrap;overflow-wrap:anywhere}.data-error{color:var(--danger)}.data-notice{color:var(--mint)}.data-settings :focus-visible{outline:2px solid var(--gold);outline-offset:3px}
@media(max-width:600px){.data-sizes{gap:10px 20px}.data-actions .data-button{flex:1}.data-confirm-actions{justify-content:stretch}.data-confirm-actions .data-button{flex:1}}
</style>
