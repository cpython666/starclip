<script setup lang="ts">
import { computed, nextTick, onMounted, onUnmounted, reactive, ref, watch } from 'vue';
import hljs from 'highlight.js/lib/core';
import javascript from 'highlight.js/lib/languages/javascript';
import typescript from 'highlight.js/lib/languages/typescript';
import python from 'highlight.js/lib/languages/python';
import rust from 'highlight.js/lib/languages/rust';
import json from 'highlight.js/lib/languages/json';
import xml from 'highlight.js/lib/languages/xml';
import css from 'highlight.js/lib/languages/css';
import bash from 'highlight.js/lib/languages/bash';
import sql from 'highlight.js/lib/languages/sql';
import markdown from 'highlight.js/lib/languages/markdown';
import go from 'highlight.js/lib/languages/go';
import java from 'highlight.js/lib/languages/java';
import HandwritingPanel from './HandwritingPanel.vue';
import type { Clip, Group } from '../types';
import { api } from '../api';
import { languages, formatJson, markdownCode, safeFilename } from '../snippets';
import { defaultCodeCardStyle, renderCodeCard } from '../code-card';

Object.entries({ javascript, typescript, python, rust, json, html: xml, css, bash, sql, markdown, go, java }).forEach(([name, definition]) => hljs.registerLanguage(name, definition));
const props = withDefaults(defineProps<{ clip: Clip | null; groups: Group[]; initialMode?: 'code' | 'handwriting' }>(), { initialMode: 'code' });
const emit = defineEmits<{ saved: []; close: [] }>();
const dialog = ref<HTMLDialogElement>();
const mode = ref<'code' | 'handwriting'>(props.initialMode);
const title = ref(props.clip?.title || (props.initialMode === 'handwriting' ? '写给星空的信' : '一页星码'));
// Switching modes shares the user's original text; neither tab replaces or rewrites it.
const content = ref(props.clip?.content ?? (props.initialMode === 'handwriting' ? '让散落的灵感，\n在这里连成星座。' : 'fn main() {\n    println!("Hello, stars. ✦");\n}'));
const language = ref(props.clip?.language || (props.clip || props.initialMode === 'handwriting' ? 'plaintext' : 'rust'));
const groupId = ref<number | null>(props.clip?.groupId ?? null);
const style = reactive({ ...defaultCodeCardStyle });
const busy = ref(false);
const error = ref('');
const notice = ref('');
const preview = ref<{ url: string; width: number; height: number } | null>(null);
const previewError = ref('');
let previewTimer: ReturnType<typeof setTimeout> | undefined;
const modifier = navigator.platform.toLowerCase().includes('mac') ? '⌘' : 'Ctrl';
const highlighted = computed(() => {
  if (content.value.length > 100000 || language.value === 'plaintext' || !hljs.getLanguage(language.value)) return escape(content.value);
  return hljs.highlight(content.value, { language: language.value, ignoreIllegals: true }).value;
});
function escape(text: string) { return text.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;'); }
function makePreview() {
  clearTimeout(previewTimer);
  if (mode.value !== 'code') return null;
  try {
    const result = renderCodeCard(title.value, content.value, language.value, highlighted.value, style);
    preview.value = result; previewError.value = '';
    return result;
  } catch (e) { preview.value = null; previewError.value = String(e instanceof Error ? e.message : e); return null; }
}
function schedulePreview() { clearTimeout(previewTimer); previewTimer = setTimeout(makePreview, 160); }
watch([title, content, language, mode], schedulePreview);
watch(style, schedulePreview);
watch(mode, () => { error.value = ''; notice.value = ''; });
onMounted(async () => { await nextTick(); dialog.value?.showModal(); makePreview(); });
onUnmounted(() => clearTimeout(previewTimer));
async function action(fn: () => Promise<void>) {
  if (busy.value) return;
  busy.value = true; error.value = ''; notice.value = '';
  try { await fn(); } catch (e) { error.value = String(e); } finally { busy.value = false; }
}
function save() {
  if (mode.value !== 'code' || !content.value.trim()) return;
  return action(async () => { await api.saveCode(props.clip?.id ?? null, title.value, content.value, language.value, groupId.value); emit('saved'); emit('close'); });
}
function format() { try { content.value = formatJson(content.value); error.value = ''; } catch { error.value = 'JSON 格式有误，请检查后再格式化。'; } }
function exportMarkdown() {
  return action(async () => { if (await api.saveExport(markdownCode(title.value, content.value, language.value), 'md', `${safeFilename(title.value)}.md`)) notice.value = 'Markdown 已导出'; });
}
function insertTab(event: KeyboardEvent) {
  if (event.key !== 'Tab' || mode.value !== 'code') return;
  event.preventDefault();
  const field = event.target as HTMLTextAreaElement;
  const start = field.selectionStart, end = field.selectionEnd;
  content.value = content.value.slice(0, start) + '  ' + content.value.slice(end);
  nextTick(() => { field.selectionStart = field.selectionEnd = start + 2; });
}
function exportImage() {
  return action(async () => {
    const image = makePreview();
    if (!image) throw new Error(previewError.value || '暂时无法生成卡片');
    if (await api.saveExport(image.url.split(',')[1]!, 'png', `${safeFilename(title.value)}.png`)) notice.value = '星码卡已导出，与当前预览一致';
  });
}
</script>

<template>
  <dialog ref="dialog" class="code-studio" @cancel.prevent="emit('close')" @click="event => { if (event.target === dialog) emit('close'); }" @keydown.meta.s.prevent="save" @keydown.ctrl.s.prevent="save">
    <div class="studio-heading"><div><span class="studio-eyebrow">A LITTLE PIECE OF YOUR UNIVERSE</span><h2>星绘坊 <span>✦</span></h2><p>让代码有星光，让文字有温度。</p></div><button class="studio-close" aria-label="关闭星绘坊" @click="emit('close')">×</button></div>
    <div class="studio-tabs" role="tablist" aria-label="卡片类型"><button id="code-tab" role="tab" :aria-selected="mode === 'code'" aria-controls="studio-workspace" :class="{ active: mode === 'code' }" @click="mode = 'code'"><span aria-hidden="true">〈/〉</span>星码卡<em>代码卡片</em></button><button id="handwriting-tab" role="tab" :aria-selected="mode === 'handwriting'" aria-controls="studio-workspace" :class="{ active: mode === 'handwriting' }" @click="mode = 'handwriting'"><span aria-hidden="true">✎</span>月光笺<em>手写纸笺</em></button><small>切换样式，正文保留</small></div>
    <div id="studio-workspace" role="tabpanel" :aria-labelledby="mode === 'code' ? 'code-tab' : 'handwriting-tab'">
      <div class="studio-meta" :class="{ 'handwriting-meta': mode === 'handwriting' }"><label>标题<input v-model="title" maxlength="200" aria-label="卡片标题" /></label><template v-if="mode === 'code'"><label>语言<select v-model="language" aria-label="代码语言"><option v-for="[value, label] in languages" :key="value" :value="value">{{ label }}</option></select></label><label>分组<select v-model="groupId" aria-label="代码分组"><option :value="null">未分组</option><option v-for="group in groups" :key="group.id" :value="group.id">{{ group.name }}</option></select></label></template><p v-else>把文字变成一张纸笺。选择字体与纸张，再生成预览。</p></div>
      <div class="studio-panels" :class="{ 'handwriting-workspace': mode === 'handwriting' }">
        <section class="studio-editor"><div class="studio-panel-label">{{ mode === 'code' ? '编辑代码' : '纸笺正文' }}<button v-if="mode === 'code' && language === 'json'" @click="format">格式化 JSON</button><span v-else>{{ content.split('\n').length }} 行 · {{ [...content].length }} 字符</span></div><textarea v-model="content" :aria-label="mode === 'code' ? '代码内容' : '纸笺正文'" :class="{ 'handwriting-text': mode === 'handwriting' }" spellcheck="false" autocomplete="off" autocapitalize="off" @keydown="insertTab" /><p class="editor-note">{{ mode === 'code' ? 'Tab 插入两个空格 · 原文只在本机处理' : '切换卡片类型不会改写正文，也不会保存为代码。' }}</p></section>
        <section v-if="mode === 'code'" class="studio-preview"><div class="studio-panel-label">导出效果预览 <span v-if="preview">{{ preview.width }} × {{ preview.height }}</span></div><div class="canvas-preview"><img v-if="preview" :src="preview.url" :alt="`${title}，星码卡导出预览`" /><div v-else class="preview-placeholder"><span>✦</span><p>{{ previewError || '正在点亮这张卡片…' }}</p></div></div><p class="preview-note">预览即导出 · PNG 以 2 倍分辨率生成</p></section>
        <section v-else class="handwriting-preview"><HandwritingPanel :text="content" :title="title" /></section>
      </div>
      <section v-if="mode === 'code'" class="card-controls" aria-label="代码卡片样式">
        <div class="control-heading"><h3>装点这张卡片</h3><button class="reset-style" @click="Object.assign(style, defaultCodeCardStyle)">恢复默认</button></div>
        <div class="control-grid">
          <label>背景<select v-model="style.palette" aria-label="卡片背景"><option value="night">深空蓝</option><option value="nebula">星云紫</option><option value="solid">自选纯色</option></select></label>
          <label v-if="style.palette === 'solid'" class="color-control">背景颜色<div><input v-model="style.background" type="color" aria-label="自选背景颜色" /><span>{{ style.background }}</span></div></label>
          <label>横向留白 <span>{{ style.paddingX }} px</span><input v-model.number="style.paddingX" type="range" min="16" max="96" step="4" aria-label="横向留白" /></label>
          <label>纵向留白 <span>{{ style.paddingY }} px</span><input v-model.number="style.paddingY" type="range" min="16" max="96" step="4" aria-label="纵向留白" /></label>
          <label>窗头<select v-model="style.windowStyle" aria-label="窗头样式"><option value="mac">macOS 三色圆点</option><option value="minimal">简洁标题</option></select></label>
          <div class="control-checks"><label><input v-model="style.lineNumbers" type="checkbox" />显示行号</label><label><input v-model="style.shadow" type="checkbox" />卡片阴影</label></div>
          <label class="watermark-control">水印 <span>留空即隐藏</span><input v-model="style.watermark" maxlength="80" aria-label="卡片水印" placeholder="写下一句属于你的话" /></label>
          <label>水印不透明度 <span>{{ Math.round(style.watermarkOpacity * 100) }}%</span><input v-model.number="style.watermarkOpacity" type="range" min="0" max="1" step="0.05" aria-label="水印不透明度" :disabled="!style.watermark.trim()" /></label>
        </div>
      </section>
    </div>
    <div v-if="mode === 'code'" class="studio-message" aria-live="polite"><span v-if="error" class="studio-error">{{ error }}</span><span v-else-if="notice">{{ notice }}</span><span v-else-if="previewError" class="studio-error">{{ previewError }}</span><span v-else>本地保存 · 不上传代码 · 新建片段收入星码集，并自动星标</span></div>
    <footer v-if="mode === 'code'" class="studio-footer"><div class="studio-export"><button :disabled="busy || !content.trim()" @click="exportMarkdown">导出 Markdown</button><button :disabled="busy || !content.trim() || !!previewError" @click="exportImage">导出星码卡 PNG</button></div><button class="studio-save" :disabled="busy || !content.trim()" @click="save">{{ busy ? '处理中…' : props.clip ? '保存修改' : '收入星码集' }} <kbd>{{ modifier }} S</kbd></button></footer>
  </dialog>
</template>

<style scoped>
.code-studio{width:min(1150px,calc(100vw - 48px));max-height:calc(100vh - 48px);box-sizing:border-box;border:1px solid #334057;border-radius:18px;padding:27px;background:#111723;color:#dce3ee;box-shadow:0 30px 100px #0008;overflow:auto}.code-studio::backdrop{background:#030711bd;backdrop-filter:blur(7px)}.studio-heading{display:flex;align-items:flex-start;justify-content:space-between;margin-bottom:21px}.studio-eyebrow{font:9px/1.5 monospace;letter-spacing:2px;color:#8e9bb0}.studio-heading h2{font-size:24px;margin:8px 0;font-weight:500}.studio-heading h2 span{color:#e9c98b;margin-left:10px}.studio-heading p{font-size:11px;color:#8895ab;margin:0}.studio-close{font-size:25px!important;background:transparent!important;border:0!important;color:#8a96aa!important;padding:0 8px!important}.code-studio input,.code-studio select{box-sizing:border-box;border:1px solid #303c50;background:#171f2e;color:#dce3ee;border-radius:7px;padding:9px;font-size:11px;min-width:0}.code-studio button{font-size:11px;background:#202b3c;border:1px solid #354157;color:#dce3ee;border-radius:7px;padding:9px 12px;cursor:pointer}.code-studio button:hover{background:#2d3a50}.code-studio button:disabled{opacity:.4;cursor:default}.code-studio :focus-visible{outline:2px solid #e9c98b;outline-offset:3px}.studio-tabs{display:flex;align-items:center;gap:7px;border-bottom:1px solid #2c384c;padding-bottom:15px;margin-bottom:18px}.studio-tabs button{background:transparent;border-color:transparent;color:#90a0b7;display:flex;align-items:center;gap:7px}.studio-tabs button>span{font-size:15px;color:#9caec8}.studio-tabs em{font-style:normal;font-size:9px;color:#8495ae}.studio-tabs button.active{background:#d7bd8d12;border-color:#d7bd8d40;color:#e2c792}.studio-tabs button.active>span{color:#e2c792}.studio-tabs small{margin-left:auto;font-size:9px;color:#66768e;letter-spacing:.4px}.studio-meta{display:grid;grid-template-columns:2fr 1fr 1fr;gap:14px;margin-bottom:17px}.studio-meta label{font-size:10px;color:#92a0b7;display:flex;flex-direction:column;gap:7px}.studio-meta.handwriting-meta{grid-template-columns:1fr 1fr;align-items:center}.studio-meta p{color:#7d8ca3;font-size:10px;line-height:1.8;margin:10px 0 0}.studio-panels{display:grid;grid-template-columns:minmax(0,1fr) minmax(0,1fr);gap:16px}.studio-editor,.studio-preview{min-width:0;border:1px solid #2c384c;border-radius:10px;overflow:hidden}.studio-panel-label{display:flex;align-items:center;justify-content:space-between;height:40px;padding:0 14px;font-size:10px;color:#9aa9bf;border-bottom:1px solid #2b364b;gap:10px}.studio-panel-label button{font-size:10px;padding:3px 6px}.studio-panel-label span{font-family:monospace;color:#71819a;font-size:9px;white-space:nowrap}.studio-editor{display:flex;flex-direction:column;background:#0c121e}.studio-editor textarea{display:block;resize:vertical;min-height:230px;height:290px;flex:1;width:100%;box-sizing:border-box;background:#0c121e;color:#d9e3ef;border:0;border-radius:0;padding:17px;font:12px/1.8 Menlo,Consolas,"PingFang SC",monospace;tab-size:2;white-space:pre;outline:none}.studio-editor textarea.handwriting-text{white-space:pre-wrap;font:14px/2.1 "PingFang SC","Microsoft YaHei",sans-serif;min-height:350px}.editor-note{font-size:9px;line-height:1.6;color:#5e708d;padding:9px 14px;border-top:1px solid #243044;margin:0}.studio-preview{background:#0c121c;display:flex;flex-direction:column}.canvas-preview{padding:17px;max-height:333px;min-height:230px;overflow:auto;flex:1;display:flex;align-items:flex-start;justify-content:center;background-image:radial-gradient(#a6b5cc12 .7px,transparent .7px);background-size:12px 12px}.canvas-preview img{display:block;width:100%;height:auto;box-shadow:0 4px 15px #0002}.preview-placeholder{align-self:center;max-width:340px;display:flex;align-items:center;flex-direction:column;padding:18px;color:#8495ae;text-align:center;gap:15px}.preview-placeholder>span{font-size:28px;color:#c4ac80}.preview-placeholder p{font-size:11px;line-height:1.8}.preview-note{font-size:9px;color:#5e708d;padding:9px 14px;border-top:1px solid #243044;margin:0}.handwriting-preview{min-width:0}.handwriting-workspace{align-items:start}.handwriting-workspace .studio-editor{position:sticky;top:0}.card-controls{border:1px solid #29364b;border-radius:10px;margin-top:17px;padding:14px 16px;background:#141c29}.control-heading{display:flex;align-items:center;justify-content:space-between;margin-bottom:14px}.control-heading h3{font-size:10px;font-weight:500;color:#c4b18b}.code-studio .reset-style{border:0;background:none;padding:0;font-size:9px;color:#788ba6}.control-grid{display:grid;grid-template-columns:repeat(4,minmax(0,1fr));gap:15px 18px}.control-grid>label{display:flex;flex-wrap:wrap;align-items:center;font-size:9px;color:#9ba9bd;gap:7px;align-content:start}.control-grid>label>span{font-size:8px;color:#61728c;margin-left:auto}.control-grid>label>select,.control-grid>label>input{width:100%}.control-grid input[type=range]{padding:0;height:25px;accent-color:#d7bd8d;border:0;background:none}.control-grid input[type=range]:disabled{opacity:.35}.control-checks{display:flex;flex-direction:column;justify-content:center;gap:12px;min-height:50px}.control-checks label{display:flex;align-items:center;gap:7px;color:#9ba9bd;font-size:10px;cursor:pointer}.control-checks input{margin:0;width:13px;height:13px;accent-color:#d7bd8d}.control-grid .watermark-control{grid-column:span 2}.color-control>div{display:flex;align-items:center;gap:9px;width:100%;font:10px monospace;color:#7e90ab}.code-studio .color-control input{padding:3px;width:35px;height:33px;cursor:pointer}.studio-message{min-height:17px;padding-top:14px;font-size:10px;color:#8796ae;line-height:1.6}.studio-error{color:#eeaaa3}.studio-footer{display:flex;justify-content:space-between;align-items:center;gap:10px;margin-top:14px}.studio-export{display:flex;gap:8px}.code-studio .studio-save{background:#e4c38c;color:#151a25;border-color:#e4c38c;font-weight:600}.studio-save kbd{font:9px monospace;margin-left:10px;opacity:.6;border-color:#151a2520;color:inherit}@media(max-width:900px){.code-studio{padding:22px;width:calc(100vw - 32px);max-height:calc(100vh - 32px)}.studio-panels{grid-template-columns:1fr}.studio-editor textarea{height:215px;min-height:180px}.studio-editor textarea.handwriting-text{height:230px;min-height:180px}.canvas-preview{max-height:420px;min-height:220px;padding:22px}.canvas-preview img{max-width:700px}.control-grid{grid-template-columns:repeat(3,minmax(0,1fr))}.handwriting-workspace .studio-editor{position:static}.studio-meta.handwriting-meta{grid-template-columns:1fr}.studio-meta.handwriting-meta p{margin:0;font-size:10px}.handwriting-preview{width:100%}}@media(max-width:600px){.code-studio{padding:17px;width:calc(100vw - 18px);max-height:calc(100vh - 18px);border-radius:12px}.studio-heading h2{font-size:22px}.studio-eyebrow{font-size:8px;letter-spacing:1px}.studio-meta{grid-template-columns:1fr 1fr;gap:12px}.studio-meta>label:first-child{grid-column:1/-1}.studio-tabs small{display:none}.studio-tabs button{padding:8px 10px;font-size:10px}.control-grid{grid-template-columns:repeat(2,minmax(0,1fr));gap:14px}.studio-footer{flex-wrap:wrap;justify-content:flex-end}.studio-export{flex:1;flex-wrap:wrap}.studio-export button{font-size:10px;padding:8px 9px}.studio-save kbd{display:none}.canvas-preview{padding:12px}.card-controls{padding:13px}.studio-tabs{margin-bottom:15px}}
</style>
