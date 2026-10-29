<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from 'vue';
import { errorText, t } from '../i18n';
import { getStartupPreferences, initialStartupPreferences, setStartupPreferences, startupDesktopAvailable } from '../startup-api';
import type { StartupPreferences } from '../startup-api';

const props = defineProps<{ disabled?: boolean }>();
const emit = defineEmits<{ busy: [value: boolean]; quit: [] }>();
const preferences = ref(initialStartupPreferences());
const loaded = ref(false);
const working = ref(false);
const error = ref('');
const notice = ref('');
let disposed = false;
const blocked = computed(() => props.disabled || working.value || !startupDesktopAvailable);
const canRelease = computed(() => !working.value);
defineExpose({ canRelease });
watch(working, value => emit('busy', value), { flush: 'sync' });

async function read() {
  if (disposed || working.value || !startupDesktopAvailable) return;
  working.value = true; error.value = ''; notice.value = '';
  try {
    const next = await getStartupPreferences();
    if (!disposed) { preferences.value = next; loaded.value = true; }
  } catch (reason) {
    if (!disposed) error.value = t('启动设置读取失败：{error}', { error: errorText(reason) });
  } finally { if (!disposed) working.value = false; }
}

async function toggle(key: keyof StartupPreferences) {
  if (disposed || blocked.value || !loaded.value) return;
  working.value = true; error.value = ''; notice.value = '';
  try {
    const next = await setStartupPreferences({ [key]: !preferences.value[key] });
    if (!disposed) {
      preferences.value = next;
      notice.value = t(key === 'startHidden' ? '已保存，下次启动星笺时生效。' : key === 'autoLaunch' ? '登录自启动设置已保存。' : '菜单栏 / 托盘图标设置已生效。');
    }
  } catch (reason) {
    if (!disposed) error.value = t('未能保存启动设置：{error}', { error: errorText(reason) });
  } finally { if (!disposed) working.value = false; }
}
function quit() { if (!disposed && !blocked.value) emit('quit'); }
onMounted(read);
onBeforeUnmount(() => { disposed = true; emit('busy', false); });
</script>

<template>
  <section class="startup-settings" :aria-label="t('启动与后台运行')" :aria-busy="working">
    <h3>{{ t('启动与后台运行') }}</h3>
    <p class="startup-note">{{ t('三项独立即时保存，不改变剪贴记录的暂停状态。') }}</p>
    <div class="setting-row">
      <div><strong>{{ t('登录系统后自动启动') }}</strong><p id="startup-login-description">{{ t('登录系统后自动运行；系统登录项或后台运行权限也可能限制启动。') }}</p></div>
      <button type="button" role="switch" :class="['switch', { on: preferences.autoLaunch }]" :aria-checked="preferences.autoLaunch" :aria-label="t('登录系统后自动启动')" aria-describedby="startup-login-description" :disabled="blocked || !loaded" @click="toggle('autoLaunch')"><span /></button>
    </div>
    <div class="setting-row">
      <div><strong>{{ t('启动时收起主窗口') }}</strong><p id="startup-hidden-description">{{ t('下次启动时不弹出主窗口；星伴按已保存的开关运行。重新打开应用或使用呼出快捷键可显示主窗口。') }}</p></div>
      <button type="button" role="switch" :class="['switch', { on: preferences.startHidden }]" :aria-checked="preferences.startHidden" :aria-label="t('启动时收起主窗口')" aria-describedby="startup-hidden-description" :disabled="blocked || !loaded" @click="toggle('startHidden')"><span /></button>
    </div>
    <div class="setting-row">
      <div><strong>{{ t('显示菜单栏 / 托盘图标') }}</strong><p id="startup-tray-description">{{ t('macOS 显示在菜单栏，Windows 显示在通知区域；关闭后星笺仍可在后台运行。') }}</p></div>
      <button type="button" role="switch" :class="['switch', { on: preferences.showTrayIcon }]" :aria-checked="preferences.showTrayIcon" :aria-label="t('显示菜单栏 / 托盘图标')" aria-describedby="startup-tray-description startup-reopen-description" :disabled="blocked || !loaded" @click="toggle('showTrayIcon')"><span /></button>
    </div>
    <p id="startup-reopen-description" class="startup-note">{{ t('隐藏图标后，仍可重新打开星笺应用或使用呼出快捷键显示主窗口；也可在此完全退出。') }}</p>
    <p v-if="!startupDesktopAvailable" class="startup-status">{{ t('启动设置仅在桌面版可用。浏览器不会注册登录项或更改菜单栏 / 托盘。') }}</p>
    <p v-else-if="working && !loaded" role="status" class="startup-note">{{ t('正在读取启动设置…') }}</p>
    <p v-if="error" role="alert" class="startup-error">{{ error }}</p>
    <p v-else-if="notice" role="status" class="startup-note">{{ notice }}</p>
    <div class="startup-actions">
      <button v-if="startupDesktopAvailable && error" type="button" class="button subtle" :disabled="blocked" @click="read">{{ t('重新读取') }}</button>
      <button type="button" class="button subtle" :disabled="blocked" @click="quit">{{ t('退出星笺') }}</button>
    </div>
  </section>
</template>

<style scoped>
.startup-settings{border-top:1px solid var(--line);padding-top:20px;margin-top:22px;min-width:0}.startup-settings h3{font-size:11px;font-weight:500;color:var(--gold);margin:0 0 9px}.startup-settings .setting-row p,.startup-note,.startup-status,.startup-error{font-size:10px;line-height:1.8;color:var(--muted);overflow-wrap:anywhere}.startup-note{margin:7px 0}.startup-status{padding:10px 12px;background:var(--input);border:1px solid var(--line);border-radius:8px;margin:12px 0}.startup-settings .startup-error{color:var(--danger);margin:10px 0}.startup-actions{display:flex;gap:9px;flex-wrap:wrap;margin-top:13px}.startup-actions .button{font-size:10px;padding:8px 12px}
</style>
