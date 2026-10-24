<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, ref, useId, watch } from 'vue';
import { normalizeHex } from '../color';
import Icon from './Icon.vue';

defineOptions({ inheritAttrs: false });
const props = defineProps<{ modelValue: string; label: string; disabled?: boolean }>();
const emit = defineEmits<{ 'update:modelValue': [value: string] }>();
const id = useId();
const trigger = ref<HTMLButtonElement>();
const panel = ref<HTMLElement>();
const input = ref<HTMLInputElement>();
const opened = ref(false);
const draft = ref('');
const edited = ref(false);
const position = ref({ left: '0px', top: '0px' });
const nativePopover = typeof HTMLElement !== 'undefined' && 'showPopover' in HTMLElement.prototype;
const current = computed(() => normalizeHex(props.modelValue) || '#000000');
const validDraft = computed(() => normalizeHex(draft.value));
const invalid = computed(() => edited.value && !validDraft.value);
const colors = [
  ['墨黑', '#202020'], ['午夜蓝', '#182433'], ['雾灰', '#6b7280'], ['岩棕', '#79695a'],
  ['松绿', '#365c4b'], ['湖青', '#60a9a6'], ['晴蓝', '#6c9bd1'], ['紫星', '#9977ff'],
  ['暮玫', '#b57885'], ['赤陶', '#bf7158'], ['日光', '#f3ce85'], ['鼠尾草', '#91a889'],
  ['桃粉', '#f2c6bf'], ['淡紫', '#d4c6ed'], ['象牙', '#f8f1e3'], ['雪白', '#ffffff'],
];
let ownerDialog: HTMLDialogElement | null = null;
let observer: MutationObserver | undefined;

function isDisabled() { return props.disabled || trigger.value?.matches(':disabled'); }
function detach() {
  document.removeEventListener('pointerdown', outside, true);
  document.removeEventListener('scroll', scrolled, true);
  window.removeEventListener('resize', place);
  ownerDialog?.removeEventListener('close', ownerClosed);
  ownerDialog = null;
  observer?.disconnect(); observer = undefined;
}
function close(restoreFocus = true) {
  if (!opened.value) return;
  opened.value = false;
  detach();
  if (nativePopover) {
    if (panel.value?.matches(':popover-open')) panel.value.hidePopover();
  } else if ((panel.value as HTMLDialogElement | undefined)?.open) {
    (panel.value as HTMLDialogElement).close();
  }
  if (restoreFocus && trigger.value?.isConnected && !isDisabled()) trigger.value.focus({ preventScroll: true });
}
function ownerClosed() { close(false); }
function outside(event: Event) {
  const target = event.target as Node | null;
  if (target && !panel.value?.contains(target) && !trigger.value?.contains(target)) close();
}
function scrolled(event: Event) {
  if (!(event.target instanceof Node) || !panel.value?.contains(event.target)) close();
}
function place() {
  if (!opened.value || !trigger.value || !panel.value) return;
  const anchor = trigger.value.getBoundingClientRect();
  const popup = panel.value.getBoundingClientRect();
  const width = document.documentElement.clientWidth;
  const height = window.innerHeight;
  const below = anchor.bottom + 7;
  const top = below + popup.height <= height - 10 ? below : anchor.top - popup.height - 7;
  position.value = {
    left: `${Math.max(10, Math.min(anchor.left, width - popup.width - 10))}px`,
    top: `${Math.max(10, Math.min(top, height - popup.height - 10))}px`,
  };
}
function ancestorChanged() {
  const button = trigger.value;
  if (!button || isDisabled() || !button.getClientRects().length || getComputedStyle(button).visibility === 'hidden') close(false);
}
async function toggle() {
  if (opened.value) { close(); return; }
  if (isDisabled() || !panel.value) return;
  draft.value = current.value;
  edited.value = false;
  if (nativePopover) panel.value.showPopover();
  else (panel.value as HTMLDialogElement).showModal();
  opened.value = true;
  await nextTick();
  if (!opened.value) return;
  place();
  input.value?.focus({ preventScroll: true });
  input.value?.select();
  document.addEventListener('pointerdown', outside, true);
  document.addEventListener('scroll', scrolled, true);
  window.addEventListener('resize', place);
  ownerDialog = trigger.value?.closest('dialog') || null;
  ownerDialog?.addEventListener('close', ownerClosed);
  observer = new MutationObserver(ancestorChanged);
  for (let ancestor: HTMLElement | null = trigger.value || null; ancestor; ancestor = ancestor.parentElement) {
    observer.observe(ancestor, { attributes: true, attributeFilter: ['disabled', 'hidden', 'style', 'class', 'open'] });
  }
}
function apply(value = draft.value) {
  if (isDisabled()) { close(false); return; }
  const normalized = normalizeHex(value);
  edited.value = true;
  if (!normalized) { input.value?.focus(); return; }
  emit('update:modelValue', normalized);
  close();
}
function keydown(event: KeyboardEvent) {
  if (event.isComposing) return;
  if (event.key === 'Escape') { event.preventDefault(); close(); }
  else if (event.key === 'Enter' && event.target === input.value) { event.preventDefault(); apply(); }
}
function panelPointerDown(event: PointerEvent) {
  const button = event.target instanceof Element ? event.target.closest('button') : null;
  if (event.button === 0 && button && panel.value?.contains(button) && !button.matches(':disabled')) {
    // WebKit does not always focus clicked buttons. Keep focus inside until click
    // commits the color, instead of letting focusout dismiss the panel first.
    event.preventDefault();
    button.focus({ preventScroll: true });
  }
  if (nativePopover || event.target !== panel.value || !panel.value) return;
  const rect = panel.value.getBoundingClientRect();
  if (event.clientX < rect.left || event.clientX > rect.right || event.clientY < rect.top || event.clientY > rect.bottom) close();
}
function nativeClosed(event: Event) {
  if (event.type === 'close' || (event as ToggleEvent).newState === 'closed') close();
}
function focusLeft() {
  if (!opened.value) return;
  queueMicrotask(() => {
    const active = document.activeElement;
    if (opened.value && active !== trigger.value && !panel.value?.contains(active)) close(false);
  });
}
watch(() => props.disabled, value => { if (value) close(false); });
onBeforeUnmount(() => { close(false); detach(); });
</script>

<template>
  <span class="ui-color" v-bind="$attrs" @focusout="focusLeft">
    <button ref="trigger" type="button" class="ui-color-trigger" :disabled="props.disabled"
      :aria-label="`${label}，当前颜色 ${current}`" aria-haspopup="dialog" :aria-expanded="opened"
      :aria-controls="`${id}-palette`" @click.stop="toggle" @keydown.stop>
      <span class="ui-color-swatch" :style="{ backgroundColor: current }" aria-hidden="true" />
      <span class="ui-color-value">{{ current.toUpperCase() }}</span>
      <Icon name="chevron" :size="12" class="ui-color-chevron" />
    </button>
    <component :is="nativePopover ? 'div' : 'dialog'" :id="`${id}-palette`" ref="panel"
      class="ui-color-panel" :popover="nativePopover ? 'manual' : undefined" :data-open="opened"
      :style="position" role="dialog" :aria-labelledby="`${id}-title`" :aria-modal="nativePopover ? undefined : true"
      @keydown.stop="keydown" @pointerdown="panelPointerDown" @cancel.prevent.stop="close()" @close="nativeClosed" @toggle="nativeClosed" @click.stop>
      <div class="ui-color-heading"><strong :id="`${id}-title`">{{ label }}</strong><button type="button" class="ui-color-close" aria-label="关闭颜色选择" @click="close()"><Icon name="close" :size="14" /></button></div>
      <div class="ui-color-palette" role="group" aria-label="常用颜色">
        <button v-for="[name, color] in colors" :key="color" type="button" class="ui-color-preset"
          :style="{ backgroundColor: color }" :aria-label="`${name} ${color}`" :title="`${name} ${color}`"
          :aria-pressed="current === color" :disabled="props.disabled" @click="apply(color)">
          <span v-if="current === color" class="ui-color-selected" aria-hidden="true">✓</span>
        </button>
      </div>
      <label class="ui-color-field" :for="`${id}-hex`">HEX 色值<span>支持任意六位颜色</span></label>
      <div class="ui-color-entry">
        <span class="ui-color-preview" :style="{ backgroundColor: validDraft || current }" aria-hidden="true" />
        <input :id="`${id}-hex`" ref="input" v-model="draft" class="ui-color-hex" type="text" inputmode="text"
          autocomplete="off" autocapitalize="off" spellcheck="false" placeholder="#F8F1E3" :disabled="props.disabled"
          :aria-invalid="invalid" :aria-describedby="`${id}-hint`" @input="edited = true" />
      </div>
      <p :id="`${id}-hint`" class="ui-color-hint" :class="{ invalid }" aria-live="polite">{{ invalid ? '请输入六位十六进制色值，例如 #A3B8D4。' : 'Enter 确认 · Escape 取消' }}</p>
      <div class="ui-color-actions"><button type="button" class="ui-color-cancel" @click="close()">取消</button><button type="button" class="ui-color-apply" :disabled="props.disabled || !validDraft" @click="apply()">使用颜色</button></div>
    </component>
  </span>
</template>

<style scoped>
.ui-color{display:inline-block;min-width:0;max-width:100%;vertical-align:middle;font:inherit;color:var(--text)}
.ui-color-trigger{display:flex;align-items:center;gap:9px;width:100%;min-height:36px;padding:7px 10px;border:1px solid var(--line);border-radius:7px;background:var(--input);color:var(--text);font:inherit;cursor:pointer;text-align:left;transition:border-color .15s,background .15s}.ui-color-trigger:hover:not(:disabled){border-color:var(--gold);background:var(--hover)}.ui-color-swatch{width:20px;height:20px;flex-shrink:0;border:1px solid var(--line);border-radius:5px}.ui-color-value{font:11px ui-monospace,SFMono-Regular,Consolas,monospace;letter-spacing:.3px;flex:1}.ui-color-chevron{color:var(--muted);transform:rotate(90deg);flex-shrink:0}.ui-color-trigger:disabled{opacity:.45;cursor:default}
.ui-color-panel{position:fixed;inset:auto;margin:0;width:278px;max-width:calc(100vw - 20px);max-height:calc(100vh - 20px);box-sizing:border-box;padding:14px;border:1px solid var(--line);border-radius:12px;background:var(--panel);color:var(--text);box-shadow:var(--shadow);font-family:inherit;overflow:auto;overscroll-behavior:contain}.ui-color-panel:not([data-open="true"]){display:none}.ui-color-panel::backdrop{background:transparent}.ui-color-heading{display:flex;align-items:center;justify-content:space-between;gap:12px;margin-bottom:13px}.ui-color-heading strong{font-size:12px;font-weight:500}.ui-color-close{display:grid;place-items:center;padding:4px;border:0;border-radius:4px;background:transparent;color:var(--muted);cursor:pointer}.ui-color-close:hover{color:var(--text);background:var(--hover)}
.ui-color-palette{display:grid;grid-template-columns:repeat(8,minmax(0,1fr));gap:7px;margin-bottom:16px}.ui-color-preset{display:grid;place-items:center;aspect-ratio:1;border:1px solid var(--line);border-radius:5px;padding:0;cursor:pointer}.ui-color-preset:hover{transform:translateY(-1px);outline:1px solid var(--gold);outline-offset:2px}.ui-color-preset[aria-pressed="true"]{outline:2px solid var(--gold);outline-offset:2px}.ui-color-selected{display:grid;place-items:center;width:15px;height:15px;border-radius:50%;font-size:10px;font-weight:600;background:var(--panel);color:var(--text)}
.ui-color-field{display:flex;align-items:center;justify-content:space-between;gap:8px;margin:0 0 7px;color:var(--text);font-size:10px}.ui-color-field>span{color:var(--muted);font-size:9px}.ui-color-entry{display:flex;align-items:center;gap:8px}.ui-color-preview{width:32px;height:32px;border:1px solid var(--line);border-radius:6px;flex-shrink:0}.ui-color-panel .ui-color-hex{min-width:0;width:100%;height:34px;box-sizing:border-box;border:1px solid var(--line);border-radius:6px;padding:7px 9px;background:var(--input);color:var(--text);font:12px ui-monospace,SFMono-Regular,Consolas,monospace}.ui-color-panel .ui-color-hex[aria-invalid="true"]{border-color:var(--danger)}.ui-color-hint{min-height:16px;margin:7px 0 13px;color:var(--muted);font-size:9px;line-height:1.7}.ui-color-hint.invalid{color:var(--danger)}.ui-color-actions{display:flex;justify-content:flex-end;gap:8px}.ui-color-actions button{border:1px solid var(--line);border-radius:6px;padding:7px 10px;font:inherit;font-size:10px;cursor:pointer}.ui-color-cancel{background:transparent;color:var(--muted)}.ui-color-apply{background:var(--gold);border-color:var(--gold)!important;color:var(--on-accent)}.ui-color-panel button:disabled,.ui-color-panel input:disabled{opacity:.45;cursor:default}.ui-color :focus-visible{outline:2px solid var(--gold);outline-offset:3px}
@media(prefers-reduced-motion:reduce){.ui-color-trigger{transition:none}.ui-color-preset:hover{transform:none}}
</style>
