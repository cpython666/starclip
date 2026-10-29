<script setup lang="ts">
import { errorText, formatNumber, t } from '../i18n';
import { computed, onBeforeUnmount, onMounted, ref, watch } from 'vue';
import { api, isDesktop } from '../api';
import { safeFilename } from '../snippets';
import { createHandwritingPreview } from '../handwriting-preview';
import type { HandwritingPreviewState, HandwritingSnapshot } from '../handwriting-preview';
import type { HandwritingAssets, HandwritingOptions } from '../types';
import UiSelect from './UiSelect.vue';
import UiColor from './UiColor.vue';

const props = defineProps<{ text: string; title: string }>();
const assets = ref<HandwritingAssets>({ available: false, path: null, fonts: [], backgrounds: [], error: null });
const loading = ref(true);
const operation = ref<'download' | 'export' | null>(null);
const downloadingFont = ref('');
const renderState = ref<HandwritingPreviewState>('idle');
const error = ref('');
const renderError = ref('');
const notice = ref('');
const font = ref('');
const fontSearch = ref('');
const fontFilter = ref<'all' | 'downloaded'>('all');
const handwritingOnly = ref(false);
const background = ref('solid:#f8f1e3:1200:1600');
const fontSize = ref(30);
const ink = ref('#24324a');
const padding = ref(100);
const preview = ref('');
const renderedKey = ref('');
let disposed = false;
const downloadedFonts = computed(() => assets.value.fonts.filter(item => item.downloaded));
const currentFont = computed(() => downloadedFonts.value.find(item => item.path === font.value));
const visibleFonts = computed(() => {
  const query = fontSearch.value.trim().toLowerCase();
  return assets.value.fonts.filter(item => (fontFilter.value === 'all' || item.downloaded)
    && (!handwritingOnly.value || item.tags.includes('手写体'))
    && (!query || item.name.toLowerCase().includes(query) || t(item.name).toLowerCase().includes(query)));
});
const emptyFontMessage = computed(() => fontFilter.value === 'downloaded' && !downloadedFonts.value.length
  ? '还没有下载字体，切换到全部字体选择喜欢的一款。'
  : handwritingOnly.value ? '当前手写体筛选下没有匹配的字体，试试其他名称、下载状态或关闭手写体筛选。'
    : '没有找到匹配的字体，试试其他名称或切换到全部字体。');
function fontSizeLabel(bytes: number) { return `${formatNumber(bytes / (1024 * 1024), { maximumFractionDigits: 1 })} MiB`; }
const backgroundOptions = computed(() => [
  { value: 'solid:#f8f1e3:1200:1600', label: t('暖白信纸') },
  { value: 'solid:#ffffff:1200:1600', label: t('纯白纸张') },
  ...assets.value.backgrounds.filter(item => !['solid:#f8f1e3:1200:1600', 'solid:#ffffff:1200:1600'].includes(item.path)).map(item => ({ value: item.path, label: t(item.name) })),
]);
const options = computed<HandwritingOptions>(() => ({ text: props.text, font: font.value, background: background.value, fontSize: fontSize.value, ink: ink.value, padding: padding.value }));
const tooLong = computed(() => [...props.text].length > 10000);
const request = computed<HandwritingSnapshot | null>(() => {
  if (!isDesktop || loading.value || !assets.value.available
    || !props.text.trim() || tooLong.value || !downloadedFonts.value.some(item => item.path === font.value)) return null;
  return { key: JSON.stringify([assets.value.path, options.value]), options: { ...options.value } };
});
const stale = computed(() => !!preview.value && renderedKey.value !== request.value?.key);
const canExport = computed(() => !!request.value && !!preview.value && !stale.value && operation.value === null && renderState.value === 'idle');
const scheduler = createHandwritingPreview(api.renderHandwriting, {
  state: value => { renderState.value = value; },
  result: (png, key) => { preview.value = png; renderedKey.value = key; renderError.value = ''; },
  error: reason => { renderError.value = errorText(reason); },
});
watch(() => request.value?.key ?? null, () => {
  const value = request.value;
  if (!value) { preview.value = ''; renderedKey.value = ''; }
  renderError.value = ''; notice.value = '';
  scheduler.update(value);
}, { immediate: true, flush: 'sync' });
function setAssets(value: HandwritingAssets) {
  assets.value = value;
  if (!value.fonts.some(item => item.downloaded && item.path === font.value)) font.value = value.fonts.find(item => item.downloaded)?.path || '';
  if (!background.value.startsWith('solid:') && !value.backgrounds.some(item => item.path === background.value)) background.value = 'solid:#f8f1e3:1200:1600';
}
async function loadAssets() {
  if (disposed || operation.value) return;
  loading.value = true; error.value = '';
  try { const value = await api.handwritingAssets(); if (!disposed) setAssets(value); }
  catch (reason) { if (!disposed) error.value = errorText(reason); }
  finally { if (!disposed) loading.value = false; }
}
onMounted(loadAssets);
onBeforeUnmount(() => { disposed = true; scheduler.dispose(); });
async function selectFont(id: string) {
  if (disposed || operation.value || loading.value || !isDesktop) return;
  const selected = assets.value.fonts.find(item => item.id === id);
  if (!selected) return;
  error.value = ''; notice.value = '';
  if (selected.downloaded) { font.value = selected.path; return; }
  operation.value = 'download'; downloadingFont.value = id;
  try {
    const value = await api.downloadHandwritingFont(id);
    if (disposed) return;
    setAssets(value);
    const installed = value.fonts.find(item => item.id === id && item.downloaded);
    if (!installed) throw new Error('字体尚未安装完成，请重试。');
    font.value = installed.path;
    notice.value = t('字体已下载，可离线使用。');
  } catch (reason) { if (!disposed) error.value = errorText(reason); }
  finally { if (!disposed) { operation.value = null; downloadingFont.value = ''; } }
}
function retryPreview() {
  if (!request.value || renderState.value !== 'idle' || operation.value) return;
  renderError.value = ''; notice.value = '';
  scheduler.update(request.value, 0);
}
async function exportImage() {
  if (!canExport.value) return;
  const png = preview.value, filename = `${safeFilename(props.title || t('月光笺'))}.png`;
  operation.value = 'export'; error.value = ''; notice.value = '';
  try { if (await api.saveExport(png, 'png', filename) && !disposed) notice.value = t('月光笺已导出'); }
  catch (reason) { if (!disposed) error.value = errorText(reason); }
  finally { if (!disposed) operation.value = null; }
}
</script>

<template>
  <section class="handwriting-panel" :aria-label="t('月光笺 · 手写纸笺制作')">
    <div class="handwriting-heading"><strong>{{ t('借一纸月光，写几行心事。') }}</strong><span>{{ t('内置手写引擎 · 字体按需下载') }}</span></div>
    <p v-if="loading" class="handwriting-hint" role="status">{{ t('正在读取字体列表…') }}</p>
    <template v-else-if="assets.available">
      <p class="handwriting-hint">{{ t('安装包不含完整字体。先看预览，点击哪款才下载哪款；下载后可离线使用。') }}</p>
      <p v-if="!isDesktop" class="handwriting-hint">{{ t('浏览器仅展示字体预览，请在桌面版下载字体并生成月光笺。') }}</p>
      <div class="font-tools">
        <label class="font-search"><span>{{ t('搜索字体') }}</span><input v-model="fontSearch" type="search" :placeholder="t('输入字体名称')" :aria-label="t('搜索字体')" /></label>
        <div class="font-filters" role="group" :aria-label="t('字体筛选')"><button type="button" :aria-pressed="fontFilter === 'all'" @click="fontFilter = 'all'">{{ t('全部字体') }}</button><button type="button" :aria-pressed="fontFilter === 'downloaded'" @click="fontFilter = 'downloaded'">{{ t('已下载字体') }}</button><button type="button" :aria-pressed="handwritingOnly" @click="handwritingOnly = !handwritingOnly">{{ t('手写体') }}</button></div>
      </div>
      <div class="font-summary"><span role="status">{{ t('显示 {visible} / {total} 款 · 已下载 {downloaded} 款', { visible: formatNumber(visibleFonts.length), total: formatNumber(assets.fonts.length), downloaded: formatNumber(downloadedFonts.length) }) }}</span><span v-if="currentFont">{{ t('正在使用：{name}', { name: t(currentFont.name) }) }}</span></div>
      <div class="font-gallery" role="region" tabindex="0" :aria-label="t('选择手写字体')">
        <article v-for="item in visibleFonts" :key="item.id" class="font-card" :class="{ 'font-selected': item.downloaded && font === item.path }">
          <img :src="item.previewUrl" :alt="t('{name} 字体预览', { name: t(item.name) })" loading="lazy" />
          <div class="font-card-heading"><strong>{{ t(item.name) }}</strong><span>{{ fontSizeLabel(item.bytes) }}</span></div>
          <div class="font-card-author">{{ t('作者：{author}', { author: t(item.author) }) }}</div>
          <div class="font-card-tags"><span v-for="tag in item.tags" :key="tag">{{ t(tag) }}</span></div>
          <div class="font-card-license"><span>{{ t(item.license) }}</span><a :href="item.sourceUrl" target="_blank" rel="noopener noreferrer">{{ t('字体来源') }}</a></div>
          <button type="button" :disabled="!!operation || !isDesktop" :aria-pressed="item.downloaded && font === item.path" :aria-label="item.downloaded ? t('使用 {name}', { name: t(item.name) }) : t('下载 {name}', { name: t(item.name) })" @click="selectFont(item.id)">{{ downloadingFont === item.id ? t('正在下载并验证…') : item.downloaded ? font === item.path ? t('正在使用 · 已下载') : t('使用此字体 · 已下载') : t('下载此字体') }}</button>
        </article>
        <p v-if="!visibleFonts.length" class="font-empty">{{ t(emptyFontMessage) }}</p>
      </div>
      <div v-if="downloadedFonts.length" class="handwriting-controls">
        <div class="handwriting-field"><span>{{ t('纸张') }}</span><UiSelect v-model="background" :options="backgroundOptions" :label="t('手写纸张')" :disabled="!!operation" /></div>
        <div class="handwriting-field"><span>{{ t('墨色') }}</span><UiColor v-model="ink" :label="t('手写墨色')" :disabled="!!operation" /></div>
        <label>{{ t('字号') }} {{ fontSize }} pt<input v-model.number="fontSize" type="range" min="12" max="72" step="1" :aria-label="t('手写字号')" /></label>
        <label>{{ t('留白') }} {{ padding }} px<input v-model.number="padding" type="range" min="16" max="160" step="4" :aria-label="t('手写留白')" /></label>
      </div>
      <div class="handwriting-preview" :aria-busy="renderState !== 'idle'">
        <img v-if="preview && request" :class="{ 'preview-outdated': stale }" :src="`data:image/png;base64,${preview}`" :alt="stale ? t('上一版月光笺，正在更新，暂不可导出') : t('当前月光笺手写预览，导出使用此图片的原始尺寸')" />
        <div v-else class="paper-placeholder"><span>✧</span><p>{{ tooLong ? t('请将正文缩短至 10,000 字以内。') : !text.trim() ? t('写下想留下的话，纸笺会自动出现。') : !font ? t('选择上方喜欢的字体，下载后即可开始制作。') : renderError ? t('这次未能生成预览，可以点击下方重试。') : t('正在为这几行文字，准备一纸月光…') }}</p></div>
        <span v-if="stale" class="preview-stale">{{ renderError ? t('预览尚未更新，请重试；当前图片不可导出') : t('正在更新预览，当前图片暂不可导出') }}</span>
      </div>
      <p class="handwriting-hint">{{ t('正文与样式会在短暂停顿后自动预览。单页排版，超出纸张的文字会裁切，请检查预览后导出。') }}</p>
      <div class="handwriting-actions"><span class="auto-preview-state" role="status">{{ t('自动预览 ·') }} {{ renderState === 'rendering' ? t('正在更新…') : renderState === 'waiting' ? t('等待输入停顿…') : renderError ? t('生成失败') : preview && !stale ? t('已更新') : t('等待有效内容') }}</span><button v-if="renderError" type="button" class="render-button" :disabled="!request || renderState !== 'idle' || !!operation" @click="retryPreview">{{ t('重试预览') }}</button><button type="button" :disabled="!canExport" @click="exportImage">{{ operation === 'export' ? t('正在导出…') : t('导出月光笺 PNG') }}</button></div>
    </template>
    <div v-else class="handwriting-unavailable">
      <span>✧</span><h3>{{ t('字体列表暂不可用') }}</h3><p>{{ assets.error ? errorText(assets.error) : t('重新读取字体列表后，即可选择字体制作月光笺。') }}</p>
      <button type="button" :disabled="!!operation" @click="loadAssets">{{ t('重试读取字体') }}</button>
    </div>
    <p class="handwriting-message" aria-live="polite"><span v-if="renderError || error" role="alert">{{ renderError || error }}</span><span v-else-if="tooLong">{{ t('单张纸笺最多支持 10,000 字，请缩短内容。') }}</span><span v-else>{{ notice }}</span></p>
  </section>
</template>

<style scoped>
.handwriting-panel button{box-sizing:border-box;border:1px solid var(--line);border-radius:7px;color:var(--text);background:var(--input);font:12px system-ui,sans-serif}.handwriting-panel button{padding:9px 12px;cursor:pointer}.handwriting-panel button:disabled{opacity:.45;cursor:default}
.handwriting-panel{padding:16px;border:1px solid var(--line);border-radius:12px;background:var(--bg);min-width:0;color:var(--text)}.handwriting-heading{display:flex;flex-direction:column;gap:6px;margin-bottom:16px}.handwriting-heading strong{font-size:14px;font-weight:500;color:var(--gold)}.handwriting-heading span{font-size:10px;color:var(--muted)}.handwriting-controls{display:grid;grid-template-columns:1fr 1fr;gap:12px}.handwriting-controls label,.handwriting-controls .handwriting-field{display:flex;flex-direction:column;gap:6px;font-size:11px;color:var(--muted)}.handwriting-controls .ui-select,.handwriting-controls .ui-color{width:100%;min-width:0}.handwriting-controls input[type=range]{width:100%}.handwriting-preview{position:relative;margin-top:16px;background:var(--input);border-radius:8px;min-height:220px;max-height:340px;overflow:auto;display:flex;align-items:flex-start;justify-content:center}.handwriting-preview img{display:block;width:100%;height:auto;max-width:320px}.handwriting-preview img.preview-outdated{opacity:.5}.paper-placeholder{padding:34px 20px;text-align:center;font-size:12px;line-height:1.9;color:var(--muted)}.paper-placeholder span{font-size:32px;color:var(--gold)}.preview-stale{position:absolute;top:8px;left:8px;right:8px;text-align:center;border-radius:5px;background:var(--panel);border:1px solid var(--line);color:var(--gold);font-size:11px;padding:8px}.handwriting-hint{font-size:10px;line-height:1.7;color:var(--muted);margin:10px 0}.handwriting-actions{display:flex;align-items:center;gap:10px;flex-wrap:wrap}.auto-preview-state{font-size:10px;color:var(--muted);margin-right:auto;line-height:1.7}.render-button{background:var(--primary)!important;color:var(--on-accent)!important;border-color:var(--primary)!important}.handwriting-unavailable{min-height:280px;display:flex;flex-direction:column;justify-content:center;align-items:center;text-align:center;padding:16px;gap:10px}.handwriting-unavailable>span{font-size:38px;color:var(--gold)}.handwriting-unavailable h3{font-size:17px;margin:0;color:var(--gold)}.handwriting-unavailable p{font-size:12px;line-height:1.9;color:var(--muted);margin:0}.handwriting-unavailable small{font-size:10px;color:var(--faint)}.handwriting-message{font-size:11px;color:var(--gold);line-height:1.6;min-height:1em;margin:10px 0 0}.handwriting-panel :focus-visible{outline:2px solid var(--primary);outline-offset:2px}
.font-tools{display:flex;align-items:flex-end;gap:10px;flex-wrap:wrap}.font-search{display:flex;flex:1 1 150px;min-width:0;flex-direction:column;gap:6px;font-size:11px;color:var(--muted)}.font-search input{box-sizing:border-box;width:100%;min-width:0;border:1px solid var(--line);border-radius:7px;padding:9px 10px;background:var(--input);color:var(--text);font:12px system-ui,sans-serif}.font-filters{display:flex;gap:6px;flex-wrap:wrap}.font-filters button[aria-pressed=true]{border-color:var(--gold);color:var(--gold)}.font-summary{display:flex;justify-content:space-between;gap:6px 12px;flex-wrap:wrap;margin-top:10px;font-size:10px;line-height:1.6;color:var(--muted)}.font-empty{grid-column:1 / -1;margin:0;padding:20px 10px;text-align:center;color:var(--muted);font-size:11px;line-height:1.7}
.font-gallery{display:grid;grid-template-columns:repeat(auto-fit,minmax(min(170px,100%),1fr));align-content:start;gap:10px;margin:10px 0 16px;padding:3px;max-height:240px;overflow-y:auto;overflow-x:hidden;scrollbar-gutter:stable;overscroll-behavior-y:contain}.font-card{min-width:0;border:1px solid var(--line);border-radius:8px;padding:9px;background:var(--input);display:flex;flex-direction:column;gap:8px}.font-card.font-selected{border-color:var(--gold)}.font-card>img{width:100%;aspect-ratio:3 / 1;object-fit:contain;border-radius:4px;background:#f8f1e3}.font-card-heading,.font-card-license{display:flex;justify-content:space-between;gap:6px;flex-wrap:wrap;align-items:baseline;font-size:10px;color:var(--muted)}.font-card-heading strong{font-size:12px;font-weight:500;color:var(--text)}.font-card-license a{color:var(--gold)}.font-card>button{width:100%;margin-top:auto;font-size:11px}.font-selected>button{color:var(--gold)}
.font-card-author{font-size:10px;line-height:1.5;color:var(--muted);overflow-wrap:anywhere}.font-card-tags{display:flex;gap:5px;flex-wrap:wrap}.font-card-tags span{border:1px solid var(--line);border-radius:4px;padding:2px 5px;font-size:10px;color:var(--muted)}
</style>
