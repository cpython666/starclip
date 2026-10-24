<script setup lang="ts">
import { computed, onMounted, ref } from 'vue';
import { api, isDesktop } from '../api';
import { safeFilename } from '../snippets';
import type { HandwritingAssets, HandwritingOptions } from '../types';
import UiSelect from './UiSelect.vue';
import UiColor from './UiColor.vue';

const props = defineProps<{ text: string; title: string }>();
const assets = ref<HandwritingAssets>({ available: false, path: null, fonts: [], backgrounds: [], error: null });
const loading = ref(true);
const busy = ref(false);
const error = ref('');
const notice = ref('');
const font = ref('');
const background = ref('solid:#f8f1e3:1200:1600');
const fontSize = ref(30);
const ink = ref('#24324a');
const padding = ref(100);
const preview = ref('');
const renderedKey = ref('');
const fontOptions = computed(() => assets.value.fonts.map(item => ({ value: item.path, label: item.name })));
const backgroundOptions = computed(() => [
  { value: 'solid:#f8f1e3:1200:1600', label: '暖白信纸' },
  { value: 'solid:#ffffff:1200:1600', label: '纯白纸张' },
  ...assets.value.backgrounds.map(item => ({ value: item.path, label: item.name })),
]);
const options = computed<HandwritingOptions>(() => ({ text: props.text, font: font.value, background: background.value, fontSize: fontSize.value, ink: ink.value, padding: padding.value }));
const currentKey = computed(() => JSON.stringify([assets.value.path, options.value]));
const stale = computed(() => !!preview.value && renderedKey.value !== currentKey.value);
const tooLong = computed(() => [...props.text].length > 10000);
function setAssets(value: HandwritingAssets) {
  assets.value = value;
  if (!value.fonts.some(item => item.path === font.value)) font.value = value.fonts.find(item => item.path.includes('青叶'))?.path || value.fonts[0]?.path || '';
  if (!background.value.startsWith('solid:') && !value.backgrounds.some(item => item.path === background.value)) background.value = 'solid:#f8f1e3:1200:1600';
}
onMounted(async () => {
  try { setAssets(await api.handwritingAssets()); } catch (e) { error.value = String(e); } finally { loading.value = false; }
});
async function chooseApp() {
  if (busy.value) return;
  busy.value = true; error.value = ''; notice.value = '';
  try { setAssets(await api.chooseHandwritingApp()); } catch (e) { error.value = String(e); } finally { busy.value = false; }
}
async function render() {
  if (busy.value || !props.text.trim() || tooLong.value) return;
  busy.value = true; error.value = ''; notice.value = '';
  const key = currentKey.value;
  try {
    const png = await api.renderHandwriting({ ...options.value });
    // Editing during a render must never export the previous text as the new card.
    if (key !== currentKey.value) { notice.value = '内容已改变，请重新生成预览。'; return; }
    preview.value = png; renderedKey.value = key;
  } catch (e) { error.value = String(e); } finally { busy.value = false; }
}
async function exportImage() {
  if (busy.value || !preview.value || stale.value) return;
  busy.value = true; error.value = ''; notice.value = '';
  try { if (await api.saveExport(preview.value, 'png', `${safeFilename(props.title || '月光笺')}.png`)) notice.value = '月光笺已导出'; } catch (e) { error.value = String(e); } finally { busy.value = false; }
}
</script>

<template>
  <section class="handwriting-panel" aria-label="月光笺 · 手写纸笺制作">
    <div class="handwriting-heading"><strong>借一纸月光，写几行心事。</strong><span v-if="assets.available">{{ assets.fonts.length }} 款手写字体 · 本地生成</span></div>
    <p v-if="loading" class="handwriting-hint" role="status">正在连接本机星笺…</p>
    <template v-else-if="assets.available">
      <div class="handwriting-controls">
        <div class="handwriting-field"><span>手写字体</span><UiSelect v-model="font" :options="fontOptions" label="手写字体" :disabled="busy" /></div>
        <div class="handwriting-field"><span>纸张</span><UiSelect v-model="background" :options="backgroundOptions" label="手写纸张" :disabled="busy" /></div>
        <label>字号 {{ fontSize }} pt<input v-model.number="fontSize" type="range" min="12" max="72" step="1" aria-label="手写字号" /></label>
        <label>留白 {{ padding }} px<input v-model.number="padding" type="range" min="16" max="160" step="4" aria-label="手写留白" /></label>
        <div class="handwriting-field"><span>墨色</span><UiColor v-model="ink" label="手写墨色" :disabled="busy" /></div>
        <button class="renderer-choice" :disabled="busy" @click="chooseApp">更换星笺应用</button>
      </div>
      <div class="handwriting-preview" :aria-busy="busy">
        <img v-if="preview" :src="`data:image/png;base64,${preview}`" alt="月光笺手写预览，导出使用此图片的原始尺寸" />
        <div v-else class="paper-placeholder"><span>✧</span><p>写下想留下的话，<br />再生成一页属于你的纸笺。</p></div>
        <span v-if="stale" class="preview-stale">内容或样式已变，请重新生成</span>
      </div>
      <p class="handwriting-hint">单页排版，超出纸张的文字会裁切，请检查预览后导出。</p>
      <div class="handwriting-actions"><button class="render-button" :disabled="busy || !text.trim() || tooLong || !font" @click="render">{{ busy ? '处理中…' : preview ? '更新预览' : '生成预览' }}</button><button :disabled="busy || !preview || stale" @click="exportImage">导出月光笺 PNG</button></div>
    </template>
    <div v-else class="handwriting-unavailable">
      <span>✧</span><h3>连接你的星笺</h3><p>{{ assets.error || '选择本机星笺应用，即可在月光笺中制作手写纸笺。' }}</p>
      <button v-if="isDesktop" :disabled="busy" @click="chooseApp">选择星笺应用</button>
      <small>使用星笺 Rust 版 · 字体与纸张随应用提供</small>
    </div>
    <p class="handwriting-message" aria-live="polite"><span v-if="error" role="alert">{{ error }}</span><span v-else-if="tooLong">单张纸笺最多支持 10,000 字，请缩短内容。</span><span v-else>{{ notice }}</span></p>
  </section>
</template>

<style scoped>
.handwriting-panel button{box-sizing:border-box;border:1px solid var(--line);border-radius:7px;color:var(--text);background:var(--input);font:12px system-ui,sans-serif}.handwriting-panel button{padding:9px 12px;cursor:pointer}.handwriting-panel button:disabled{opacity:.45;cursor:default}
.handwriting-panel{padding:16px;border:1px solid var(--line);border-radius:12px;background:var(--bg);min-width:0;color:var(--text)}.handwriting-heading{display:flex;flex-direction:column;gap:6px;margin-bottom:16px}.handwriting-heading strong{font-size:14px;font-weight:500;color:var(--gold)}.handwriting-heading span{font-size:10px;color:var(--muted)}.handwriting-controls{display:grid;grid-template-columns:1fr 1fr;gap:12px}.handwriting-controls label,.handwriting-controls .handwriting-field{display:flex;flex-direction:column;gap:6px;font-size:11px;color:var(--muted)}.handwriting-controls .ui-select,.handwriting-controls .ui-color{width:100%;min-width:0}.handwriting-controls input[type=range]{width:100%}.renderer-choice{justify-self:end;align-self:center;font-size:10px!important;padding:6px 8px!important}.handwriting-preview{position:relative;margin-top:16px;background:var(--input);border-radius:8px;min-height:220px;max-height:340px;overflow:auto;display:flex;align-items:flex-start;justify-content:center}.handwriting-preview img{display:block;width:100%;height:auto;max-width:320px}.paper-placeholder{padding:34px 20px;text-align:center;font-size:12px;line-height:1.9;color:var(--muted)}.paper-placeholder span{font-size:32px;color:var(--gold)}.preview-stale{position:absolute;top:8px;left:8px;right:8px;text-align:center;border-radius:5px;background:var(--gold-dim);color:var(--gold);font-size:11px;padding:8px}.handwriting-hint{font-size:10px;line-height:1.7;color:var(--muted);margin:10px 0}.handwriting-actions{display:flex;gap:10px;flex-wrap:wrap}.render-button{background:var(--primary)!important;color:var(--on-accent)!important;border-color:var(--primary)!important}.handwriting-unavailable{min-height:280px;display:flex;flex-direction:column;justify-content:center;align-items:center;text-align:center;padding:16px;gap:10px}.handwriting-unavailable>span{font-size:38px;color:var(--gold)}.handwriting-unavailable h3{font-size:17px;margin:0;color:var(--gold)}.handwriting-unavailable p{font-size:12px;line-height:1.9;color:var(--muted);margin:0}.handwriting-unavailable small{font-size:10px;color:var(--faint)}.handwriting-message{font-size:11px;color:var(--gold);line-height:1.6;min-height:1em;margin:10px 0 0}.handwriting-panel :focus-visible{outline:2px solid var(--primary);outline-offset:2px}
</style>
