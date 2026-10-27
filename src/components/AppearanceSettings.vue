<script setup lang="ts">
import { computed, onBeforeUnmount, ref, watch } from 'vue';
import UiColor from './UiColor.vue';
import UiSelect from './UiSelect.vue';
import { appearancePresets, gradientDirections } from '../appearance';
import type { Appearance, AppearancePatch } from '../appearance';
import { appearanceDesktopAvailable, importAppearanceSticker, removeAppearanceSticker, saveAppearance } from '../appearance-api';

const props = withDefaults(defineProps<{ value: Appearance; disabled?: boolean }>(), { disabled: false });
const emit = defineEmits<{ change: [value: Appearance]; busy: [value: boolean] }>();
const working = ref(false), error = ref(false), notice = ref('');
const fileInput = ref<HTMLInputElement>();
const angleDraft = ref(props.value.angle);
const removeConfirm = ref(false);
const blocked = computed(() => props.disabled || working.value);
const backgroundChoices = [
  { value: 'theme' as const, label: '跟随基础主题' },
  { value: 'solid' as const, label: '自选纯色' },
  { value: 'gradient' as const, label: '双色渐变' },
];
const preview = computed(() => props.value.background === 'theme' ? 'var(--bg)' : props.value.background === 'solid' ? props.value.colorStart : `linear-gradient(${angleDraft.value}deg, ${props.value.colorStart}, ${props.value.colorEnd})`);
let disposed = false;
watch(() => props.value.angle, value => { angleDraft.value = value; });
watch(working, value => emit('busy', value), { flush: 'sync' });
onBeforeUnmount(() => { disposed = true; emit('busy', false); });

async function run(action: () => Promise<Appearance>, message: string) {
  if (blocked.value || disposed) return;
  working.value = true; error.value = false; notice.value = '正在保存外观…';
  try {
    const value = await action();
    if (!disposed) { emit('change', value); angleDraft.value = value.angle; notice.value = appearanceDesktopAvailable ? message : `${message} 浏览器仅在本次页面演示，刷新后还原。`; removeConfirm.value = false; }
  } catch (reason) {
    if (!disposed) { error.value = true; notice.value = `未能保存，原外观保留。${reason instanceof Error ? reason.message : String(reason)}`; angleDraft.value = props.value.angle; }
  } finally { if (!disposed) working.value = false; }
}
function save(patch: AppearancePatch) { return run(() => saveAppearance(patch), '外观已保存并生效。'); }
function setAngle(event: Event) {
  const input = event.target as HTMLInputElement;
  const value = Number(input.value);
  input.value = String(props.value.angle); angleDraft.value = props.value.angle;
  if (Number.isInteger(value) && value >= 0 && value <= 359) void save({ angle: value });
  else { angleDraft.value = props.value.angle; error.value = true; notice.value = '请输入 0–359 的整数角度。'; }
}
function toggleAccentFollow(event: Event) {
  const input = event.target as HTMLInputElement, requested = input.checked;
  input.checked = props.value.accent === null;
  void save({ accent: requested ? null : '#9977ff' });
}
function toggleSticker(event: Event) {
  const input = event.target as HTMLInputElement, requested = input.checked;
  input.checked = props.value.stickerEnabled;
  void save({ stickerEnabled: requested });
}
async function selectedFile(event: Event) {
  const input = event.target as HTMLInputElement;
  const file = input.files?.[0];
  if (file) await run(() => importAppearanceSticker(file), '贴纸已导入并启用，原文件没有改动。');
  input.value = '';
}
</script>

<template>
  <section class="appearance-settings" :aria-busy="working" aria-label="自定义配色与贴纸">
    <div class="appearance-title"><div><h3>让星笺带上你的颜色</h3><p>配色与贴纸独立保存，成功后立即生效。</p></div><button type="button" class="appearance-reset" :disabled="blocked || (!value.accent && value.background === 'theme')" @click="save({ accent: null, background: 'theme' })">恢复主题配色</button></div>
    <div class="appearance-row"><div><strong>主题强调色</strong><p>用于按钮、选中项与重点文字；会调整明度以保持可读。</p></div><UiColor :model-value="value.accent || '#9977ff'" label="自选主题强调色" :disabled="blocked" @update:model-value="save({ accent: $event })" /></div>
    <label class="appearance-check"><input type="checkbox" :checked="value.accent === null" :disabled="blocked" @change="toggleAccentFollow" />强调色跟随基础主题</label>

    <div class="appearance-background"><div class="appearance-row"><div><strong>窗口背景</strong><p>正文区域保留主题底色，让渐变装饰不影响阅读。</p></div><UiSelect :model-value="value.background" :options="backgroundChoices" label="窗口背景模式" :disabled="blocked" @update:model-value="save({ background: $event })" /></div>
      <div class="appearance-preview" :style="{ background: preview }" aria-label="窗口背景预览"><div><span>剪贴板里的小灵感</span><strong>颜色由你选择，内容依然清楚。</strong><span class="appearance-preview-button">搜索与收藏</span></div></div>
      <div v-if="value.background !== 'theme'" class="appearance-color-row"><label>{{ value.background === 'solid' ? '背景颜色' : '起点颜色' }}<UiColor :model-value="value.colorStart" label="背景起点颜色" :disabled="blocked" @update:model-value="save({ colorStart: $event })" /></label><template v-if="value.background === 'gradient'"><button type="button" class="appearance-swap" :disabled="blocked" aria-label="交换渐变颜色" title="交换颜色" @click="save({ colorStart: value.colorEnd, colorEnd: value.colorStart })">⇄</button><label>终点颜色<UiColor :model-value="value.colorEnd" label="背景终点颜色" :disabled="blocked" @update:model-value="save({ colorEnd: $event })" /></label></template></div>
      <div v-if="value.background === 'gradient'" class="appearance-angle"><div class="appearance-angle-heading"><label for="appearance-angle">渐变方向</label><label class="appearance-angle-number"><input id="appearance-angle" type="number" min="0" max="359" step="1" :value="angleDraft" :disabled="blocked" aria-label="渐变角度" @change="setAngle" /><span>°</span></label></div><input type="range" min="0" max="359" step="1" :value="angleDraft" @input="angleDraft = Number(($event.target as HTMLInputElement).value)" :disabled="blocked" aria-label="拖动调整渐变方向" @change="setAngle" /><div class="appearance-directions" role="group" aria-label="渐变方向快捷选择"><button v-for="direction in gradientDirections" :key="direction.value" type="button" :disabled="blocked" :aria-pressed="value.angle === direction.value" @click="save({ angle: direction.value })">{{ direction.label }}</button></div></div>
      <div class="appearance-presets" role="group" aria-label="渐变配色预设"><button v-for="preset in appearancePresets" :key="preset.name" type="button" :disabled="blocked" :aria-label="`使用${preset.name}渐变`" @click="save({ background: 'gradient', colorStart: preset.colorStart, colorEnd: preset.colorEnd, angle: preset.angle })"><span :style="{ background: `linear-gradient(${preset.angle}deg, ${preset.colorStart}, ${preset.colorEnd})` }" /><span>{{ preset.name }}</span></button></div>
      <p class="appearance-note">「柔粉奶油」配色参考 uiGradients 的 Broken Hearts。也可以把喜欢的两种 HEX 色值填进来，再自由调整方向。</p>
    </div>

    <div class="appearance-sticker"><div class="appearance-row"><div><strong>导入自己的贴纸</strong><p>启用时优先显示；停用后使用下方选择的内置贴纸。</p></div><button type="button" class="button subtle" :disabled="blocked" @click="fileInput?.click()">{{ value.stickerDataUrl ? '替换 PNG 贴纸' : '导入 PNG 贴纸' }}</button><input ref="fileInput" type="file" accept="image/png,.png" class="appearance-file" aria-label="选择本地 PNG 贴纸" :disabled="blocked" @change="selectedFile" /></div>
      <div v-if="value.stickerDataUrl" class="appearance-sticker-current"><div class="appearance-checker"><img :src="value.stickerDataUrl" alt="当前导入的自定义贴纸" /></div><div><strong>{{ value.stickerName || '自定义贴纸' }}</strong><label class="appearance-check"><input type="checkbox" :checked="value.stickerEnabled" :disabled="blocked" @change="toggleSticker" />使用这张贴纸</label><button type="button" class="appearance-remove" :disabled="blocked" @click="removeConfirm = true">移除已导入贴纸</button></div></div>
      <div v-if="removeConfirm" class="appearance-remove-confirm"><p>移除星笺中保存的这张贴纸？你的原始 PNG 文件会保留。</p><div><button type="button" class="button subtle" :disabled="blocked" @click="removeConfirm = false">取消</button><button type="button" class="button subtle" :disabled="blocked" @click="run(removeAppearanceSticker, '已移除导入贴纸，恢复内置贴纸选择。')">确认移除</button></div></div>
      <p class="appearance-note">静态 PNG，可保留透明背景；单张最多 5 MiB、400 万像素，边长不超过 2048。导入后缩至最长边 512 像素并保存在本机；暂不支持动画或 SVG。</p>
    </div>
    <p v-if="notice" :class="['appearance-status', { error }]" :role="error ? 'alert' : 'status'">{{ notice }}</p>
    <p v-if="!appearanceDesktopAvailable" class="appearance-note">浏览器仅演示外观，不持久保存。桌面版会随完整数据备份保留；普通历史 JSON 不包含外观设置。</p>
  </section>
</template>

<style scoped>
.appearance-settings{padding:18px 0;border-top:1px solid var(--line);margin-top:18px;min-width:0}.appearance-title,.appearance-row{display:flex;justify-content:space-between;align-items:center;gap:14px}.appearance-title{align-items:flex-start;margin-bottom:12px}.appearance-title h3{font-size:13px;font-weight:500;color:var(--gold);margin:0 0 7px}.appearance-title p,.appearance-row p{font-size:10px;color:var(--muted);line-height:1.7;margin:5px 0 0}.appearance-title>div,.appearance-row>div{min-width:0}.appearance-row{margin:15px 0 10px}.appearance-row strong{font-size:11px;font-weight:500}.appearance-row>.ui-color,.appearance-row>.ui-select{width:180px;flex-shrink:0;font-size:10px}.appearance-reset{font-size:10px;color:var(--gold);flex-shrink:0;padding:4px 0}.appearance-reset:hover{text-decoration:underline}.appearance-check{display:flex;align-items:center;gap:7px;font-size:10px;color:var(--muted);line-height:1.7;cursor:pointer}.appearance-background,.appearance-sticker{border-top:1px solid var(--line-soft);margin-top:17px;padding-top:4px}.appearance-preview{padding:20px;border-radius:12px;margin-top:13px;min-height:130px;border:1px solid var(--line);display:flex;align-items:center;justify-content:center}.appearance-preview>div{display:flex;flex-direction:column;gap:10px;background:var(--panel);color:var(--text);padding:16px 22px;border:1px solid var(--line);border-radius:9px;box-shadow:0 5px 15px #0001;min-width:0}.appearance-preview>div>span:first-child{font-size:9px;color:var(--muted)}.appearance-preview strong{font-size:11px;font-weight:500;line-height:1.5}.appearance-preview-button{font-size:9px;padding:5px 9px;border-radius:5px;background:var(--primary);color:var(--on-accent);align-self:flex-start}.appearance-color-row{display:flex;align-items:flex-end;gap:10px;margin-top:14px}.appearance-color-row>label{display:flex;flex:1;min-width:0;flex-direction:column;gap:7px;font-size:10px;color:var(--muted)}.appearance-swap{display:grid;place-items:center;width:30px;height:36px;font-size:19px;color:var(--gold);flex-shrink:0}.appearance-angle{margin-top:16px}.appearance-angle-heading{display:flex;align-items:center;justify-content:space-between;gap:12px;font-size:10px;color:var(--muted)}.appearance-angle-number{display:flex;align-items:center;gap:5px}.appearance-angle-number>input{width:65px;font-size:11px;margin:0!important;padding:6px 8px}.appearance-directions{display:grid;grid-template-columns:repeat(4,minmax(0,1fr));gap:5px;margin-top:5px}.appearance-directions button{font-size:9px;border:1px solid var(--line);padding:7px 3px;border-radius:5px;color:var(--muted)}.appearance-directions button[aria-pressed=true]{border-color:var(--gold);color:var(--gold);background:var(--gold-dim)}.appearance-presets{display:grid;grid-template-columns:repeat(5,minmax(0,1fr));gap:7px;margin-top:17px}.appearance-presets button{font-size:9px;color:var(--muted);text-align:center;min-width:0}.appearance-presets button>span:first-child{display:block;height:32px;border:1px solid var(--line);border-radius:7px;margin-bottom:6px}.appearance-presets button:hover>span:first-child{outline:1px solid var(--gold);outline-offset:2px}.appearance-note{font-size:9px;line-height:1.8;color:var(--muted);margin:12px 0 0}.appearance-sticker .button{font-size:10px;padding:8px 10px;flex-shrink:0}.appearance-file{display:none!important}.appearance-sticker-current{display:flex;align-items:center;gap:16px;padding:12px;border:1px solid var(--line);border-radius:10px;margin-top:12px;background:var(--input)}.appearance-checker{width:94px;height:80px;flex-shrink:0;display:grid;place-items:center;border:1px solid var(--line);border-radius:7px;background:repeating-conic-gradient(#aaa2 0% 25%,transparent 0% 50%) 0 0/12px 12px,var(--panel)}.appearance-checker img{max-width:86px;max-height:72px;object-fit:contain}.appearance-sticker-current>div:last-child{min-width:0}.appearance-sticker-current strong{display:block;font-size:10px;font-weight:500;overflow-wrap:anywhere;margin-bottom:10px}.appearance-remove{font-size:9px;color:var(--danger);margin-top:9px;padding:2px 0}.appearance-remove-confirm{padding:12px;border:1px solid var(--line);border-radius:8px;margin-top:10px;background:var(--panel);font-size:10px;line-height:1.7}.appearance-remove-confirm>div{display:flex;justify-content:flex-end;gap:7px;margin-top:10px}.appearance-status{font-size:10px;line-height:1.8;color:var(--mint);margin:12px 0 0;white-space:pre-wrap;overflow-wrap:anywhere}.appearance-status.error{color:var(--danger)}.appearance-settings :focus-visible{outline:2px solid var(--gold);outline-offset:3px}.appearance-settings button:disabled,.appearance-settings input:disabled{opacity:.45;cursor:default}@media(max-width:600px){.appearance-row{align-items:flex-start;flex-wrap:wrap}.appearance-row>.ui-color,.appearance-row>.ui-select{width:100%}.appearance-row>div{flex:1;min-width:160px}.appearance-title{gap:8px}.appearance-presets{grid-template-columns:repeat(3,minmax(0,1fr))}.appearance-preview{padding:14px}.appearance-preview>div{padding:14px}.appearance-sticker-current{gap:10px}.appearance-checker{width:75px}.appearance-checker img{max-width:68px}}
</style>
