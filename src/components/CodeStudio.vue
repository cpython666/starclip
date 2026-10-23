<script setup lang="ts">
import { computed, nextTick, onMounted, ref } from 'vue';
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
import type { Clip, Group } from '../types';
import { api } from '../api';
import { languages, formatJson, markdownCode, safeFilename } from '../snippets';

Object.entries({ javascript, typescript, python, rust, json, html: xml, css, bash, sql, markdown, go, java }).forEach(([name, definition]) => hljs.registerLanguage(name, definition));
const props = defineProps<{ clip: Clip | null; groups: Group[] }>();
const emit = defineEmits<{ saved: []; close: [] }>();
const dialog = ref<HTMLDialogElement>();
const title = ref(props.clip?.title || '未命名的小抄');
const content = ref(props.clip?.content || 'fn main() {\n    println!("Hello, stars. ✦");\n}');
const language = ref(props.clip?.language || (props.clip ? 'plaintext' : 'rust'));
const groupId = ref<number | null>(props.clip?.groupId ?? null);
const lines = ref(true);
const busy = ref(false);
const error = ref('');
const notice = ref('');
const palette = ref('night');
const highlighted = computed(() => {
  // highlight.js escapes source before returning markup; raw clipboard HTML never enters v-html.
  if (content.value.length > 100000) return escape(content.value);
  return language.value === 'plaintext' ? escape(content.value) : hljs.highlight(content.value, { language: language.value, ignoreIllegals: true }).value;
});
const lineNumbers = computed(() => Array.from({ length: content.value.split('\n').length }, (_, i) => i + 1).join('\n'));
function escape(text: string) { return text.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;'); }
onMounted(async () => { await nextTick(); dialog.value?.showModal(); });
async function action(fn: () => Promise<void>) { busy.value = true; error.value = ''; notice.value = ''; try { await fn(); } catch (e) { error.value = String(e); } finally { busy.value = false; } }
function save() { return action(async () => { await api.saveCode(props.clip?.id ?? null, title.value, content.value, language.value, groupId.value); emit('saved'); emit('close'); }); }
function format() { try { content.value = formatJson(content.value); error.value = ''; } catch { error.value = 'JSON 格式有误，请检查后再格式化。'; } }
function exportMarkdown() { return action(async () => { if (await api.saveExport(markdownCode(title.value, content.value, language.value), 'md', `${safeFilename(title.value)}.md`)) notice.value = 'Markdown 已导出'; }); }
function insertTab(event: KeyboardEvent) {
  if (event.key !== 'Tab') return;
  event.preventDefault();
  const field = event.target as HTMLTextAreaElement;
  const start = field.selectionStart, end = field.selectionEnd;
  content.value = content.value.slice(0, start) + '  ' + content.value.slice(end);
  nextTick(() => { field.selectionStart = field.selectionEnd = start + 2; });
}
function exportImage() { return action(async () => {
  const codeLines = content.value.replace(/\t/g, '  ').split('\n');
  if (codeLines.length > 100 || Math.max(...codeLines.map((line) => [...line].length)) > 120) throw new Error('卡片适合短代码：请控制在 100 行以内，每行不超过 120 字符。完整内容可导出 Markdown。');
  const canvas = document.createElement('canvas');
  const context = canvas.getContext('2d');
  if (!context) throw new Error('当前环境无法生成图片');
  const font = '15px Menlo, Consolas, monospace';
  context.font = font;
  const widest = Math.max(0, ...codeLines.map((line) => context.measureText(line).width));
  const width = Math.max(680, Math.ceil(widest) + (lines.value ? 180 : 144));
  const height = Math.max(300, codeLines.length * 25 + 180);
  canvas.width = width * 2; canvas.height = height * 2; context.scale(2, 2);
  const bg = context.createLinearGradient(0, 0, width, height);
  bg.addColorStop(0, palette.value === 'night' ? '#17253a' : '#3c2940'); bg.addColorStop(1, '#111520');
  context.fillStyle = bg; context.fillRect(0, 0, width, height);
  for (let i = 0; i < 45; i++) { context.fillStyle = i % 3 === 0 ? '#e9c98b66' : '#a1b7d433'; context.beginPath(); context.arc((i * 139 + 17) % width, (i * 73 + 12) % height, i % 4 === 0 ? 1.5 : .8, 0, Math.PI * 2); context.fill(); }
  context.fillStyle = '#0e131f'; context.beginPath(); context.roundRect(32, 32, width - 64, height - 76, 14); context.fill();
  ['#ef7a7a', '#e3bf79', '#8fc2a4'].forEach((color, i) => { context.fillStyle = color; context.beginPath(); context.arc(53 + i * 17, 54, 4.5, 0, Math.PI * 2); context.fill(); });
  context.fillStyle = '#cad4e3'; context.font = '13px system-ui, sans-serif';
  const displayTitle = [...title.value].slice(0, 50).join(''); context.fillText(displayTitle, 114, 59, width - 240);
  context.textAlign = 'right'; context.fillStyle = '#8896ad'; context.fillText(language.value, width - 52, 59); context.textAlign = 'left';
  context.font = font;
  const colors: Record<string, string> = { keyword: '#be9de3', string: '#a8c99b', comment: '#6d7b92', number: '#e3bf79', title: '#88bccc', built_in: '#d2af85', attr: '#88bccc', literal: '#d8a1bc', type: '#88bccc' };
  const tokens = new DOMParser().parseFromString(`<pre>${highlighted.value}</pre>`, 'text/html').querySelector('pre')!;
  let x = lines.value ? 100 : 58, row = 0;
  const left = x;
  function render(node: Node, color = '#d8e0ec') {
    if (node.nodeType === Node.TEXT_NODE) {
      const parts = (node.textContent || '').replace(/\t/g, '  ').split('\n');
      parts.forEach((part, i) => { if (i) { row++; x = left; } context!.fillStyle = color; context!.fillText(part, x, 101 + row * 25); x += context!.measureText(part).width; });
    } else {
      const element = node as Element;
      const token = [...(element.classList || [])].find((name) => name.startsWith('hljs-'))?.slice(5);
      node.childNodes.forEach((child) => render(child, token && colors[token] ? colors[token]! : color));
    }
  }
  render(tokens);
  if (lines.value) { context.fillStyle = '#46536c'; context.textAlign = 'right'; codeLines.forEach((_, i) => context.fillText(String(i + 1), 79, 101 + i * 25)); context.textAlign = 'left'; }
  context.fillStyle = '#bba77d'; context.font = '12px system-ui, sans-serif'; context.fillText('✦  星笺 STARCLIP', 36, height - 20);
  context.fillStyle = '#73829b'; context.textAlign = 'right'; context.fillText('拾起每一次灵感', width - 36, height - 20);
  if (await api.saveExport(canvas.toDataURL('image/png').split(',')[1]!, 'png', `${safeFilename(title.value)}.png`)) notice.value = '星空代码卡片已导出';
}); }
</script>

<template>
  <dialog ref="dialog" class="code-studio" @cancel.prevent="emit('close')" @click="(event) => { if (event.target === dialog) emit('close'); }" @keydown.meta.s.prevent="save" @keydown.ctrl.s.prevent="save">
    <div class="studio-heading"><div><span class="studio-eyebrow">A LITTLE PIECE OF YOUR UNIVERSE</span><h2>代码小抄 <span>✦</span></h2><p>从一次复制，变成值得收藏的代码。</p></div><button class="studio-close" aria-label="关闭代码编辑器" @click="emit('close')">×</button></div>
    <div class="studio-meta"><label>标题<input v-model="title" maxlength="200" aria-label="小抄标题" /></label><label>语言<select v-model="language" aria-label="代码语言"><option v-for="[value, label] in languages" :key="value" :value="value">{{ label }}</option></select></label><label>分组<select v-model="groupId" aria-label="小抄分组"><option :value="null">未分组</option><option v-for="group in groups" :key="group.id" :value="group.id">{{ group.name }}</option></select></label></div>
    <div class="studio-panels"><section class="studio-editor"><div class="studio-panel-label">编辑代码 <button v-if="language === 'json'" @click="format">格式化 JSON</button><span v-else>{{ content.split('\n').length }} 行</span></div><textarea v-model="content" aria-label="代码内容" spellcheck="false" autocomplete="off" autocapitalize="off" @keydown="insertTab" /></section><section class="studio-preview" :class="palette"><div class="studio-panel-label">分享卡片预览 <label class="studio-line-check"><input v-model="lines" type="checkbox" />行号</label></div><div class="code-card"><div class="code-card-title"><i /><i /><i /><span>{{ title || '未命名的小抄' }}</span><small>{{ language }}</small></div><div class="code-card-content"><pre v-if="lines" class="code-lines" aria-hidden="true">{{ lineNumbers }}</pre><pre class="code-highlight"><code v-html="highlighted" /></pre></div></div><div class="code-card-sign">✦ 星笺 STARCLIP <span>拾起每一次灵感</span></div></section></div>
    <div class="studio-message" aria-live="polite"><span v-if="error" class="studio-error">{{ error }}</span><span v-else-if="notice">{{ notice }}</span><span v-else>本地保存 · 不上传代码 · 新建小抄自动星标</span></div>
    <footer class="studio-footer"><div class="studio-export"><select v-model="palette" aria-label="卡片配色"><option value="night">深空蓝</option><option value="nebula">星云紫</option></select><button :disabled="busy || !content.trim()" @click="exportMarkdown">导出 Markdown</button><button :disabled="busy || !content.trim()" @click="exportImage">导出 PNG 卡片</button></div><button class="studio-save" :disabled="busy || !content.trim()" @click="save">{{ busy ? '处理中…' : '保存小抄' }} <kbd>⌘ S</kbd></button></footer>
  </dialog>
</template>

<style scoped>
.code-studio{width:min(1100px,calc(100vw - 48px));max-height:calc(100vh - 48px);box-sizing:border-box;border:1px solid #334057;border-radius:20px;padding:28px;background:#111723;color:#dce3ee;box-shadow:0 30px 100px #0008}.code-studio::backdrop{background:#030711bd;backdrop-filter:blur(7px)}.studio-heading{display:flex;align-items:flex-start;justify-content:space-between;margin-bottom:24px}.studio-eyebrow{font:10px/1.5 monospace;letter-spacing:2px;color:#8e9bb0}.studio-heading h2{font-size:25px;margin:8px 0}.studio-heading h2 span{color:#e9c98b;margin-left:10px}.studio-heading p{font-size:12px;color:#8895ab;margin:0}.studio-close{font-size:25px!important;background:transparent!important;border:0!important;color:#8a96aa!important;padding:0 8px!important}.studio-meta{display:grid;grid-template-columns:2fr 1fr 1fr;gap:16px;margin-bottom:20px}.studio-meta label{font-size:11px;color:#92a0b7;display:flex;flex-direction:column;gap:8px}.code-studio input,.code-studio select{box-sizing:border-box;border:1px solid #303c50;background:#171f2e;color:#dce3ee;border-radius:7px;padding:10px;font:12px inherit;min-width:0}.code-studio button{font-size:12px;background:#202b3c;border:1px solid #354157;color:#dce3ee;border-radius:7px;padding:9px 12px;cursor:pointer}.code-studio button:disabled{opacity:.4;cursor:wait}.code-studio :focus-visible{outline:2px solid #e9c98b;outline-offset:3px}.studio-panels{display:grid;grid-template-columns:1fr 1fr;gap:16px}.studio-editor,.studio-preview{min-width:0;border:1px solid #2c384c;border-radius:12px;overflow:hidden}.studio-panel-label{display:flex;align-items:center;justify-content:space-between;height:41px;padding:0 14px;font-size:11px;color:#8f9fb6;border-bottom:1px solid #2b364b}.studio-panel-label button{font-size:10px;padding:3px 6px}.studio-panel-label span{font-family:monospace;color:#65758e}.studio-editor textarea{display:block;resize:vertical;min-height:300px;height:340px;width:100%;box-sizing:border-box;background:#0c121e;color:#d9e3ef;border:0;padding:18px;font:12px/1.8 Menlo,Consolas,monospace;tab-size:2;white-space:pre;outline:none}.studio-preview{background:radial-gradient(ellipse at top left,#26364f,#151c2b)}.studio-preview.nebula{background:radial-gradient(ellipse at top left,#49324e,#1d192a)}.studio-line-check{display:flex;align-items:center;gap:5px}.studio-line-check input{accent-color:#d7bd8d}.code-card{background:#0e131f;margin:22px 18px 12px;border:1px solid #3c486244;border-radius:9px;overflow:auto;max-height:295px;min-height:242px;box-shadow:0 10px 30px #05091255}.code-card-title{display:flex;gap:5px;align-items:center;padding:13px 12px;position:sticky;left:0}.code-card-title i{width:7px;height:7px;background:#ef7a7a;border-radius:50%;flex-shrink:0}.code-card-title i:nth-child(2){background:#e3bf79}.code-card-title i:nth-child(3){background:#8fc2a4}.code-card-title span{font:10px system-ui;margin-left:8px;overflow:hidden;text-overflow:ellipsis;white-space:nowrap}.code-card-title small{color:#65738a;font:9px monospace;margin-left:auto}.code-card-content{display:flex;padding:8px 16px 18px;min-width:max-content}.code-card-content pre{margin:0;font:11px/1.8 Menlo,Consolas,monospace;white-space:pre;background:transparent;border:0;padding:0}.code-lines{color:#485774;user-select:none;text-align:right;padding-right:15px!important}.code-highlight{color:#d8e0ec}.code-card-sign{display:flex;justify-content:space-between;color:#bba77d;margin:0 19px 15px;font-size:8px;letter-spacing:1px}.code-card-sign span{color:#7c8ca6}.studio-message{min-height:17px;padding-top:15px;font-size:11px;color:#8796ae}.studio-error{color:#eeaaa3}.studio-footer{display:flex;justify-content:space-between;align-items:center;gap:10px;margin-top:16px}.studio-export{display:flex;gap:8px}.studio-save{background:#e4c38c!important;color:#151a25!important;border-color:#e4c38c!important;font-weight:600}.studio-save kbd{font:10px monospace;margin-left:10px;opacity:.6}.code-highlight :deep(.hljs-keyword){color:#be9de3}.code-highlight :deep(.hljs-string){color:#a8c99b}.code-highlight :deep(.hljs-comment){color:#6d7b92}.code-highlight :deep(.hljs-number),.code-highlight :deep(.hljs-built_in){color:#e3bf79}.code-highlight :deep(.hljs-title),.code-highlight :deep(.hljs-attr),.code-highlight :deep(.hljs-type){color:#88bccc}.code-highlight :deep(.hljs-literal){color:#d8a1bc}@media(max-width:800px){.studio-panels{grid-template-columns:1fr}.studio-preview{display:none}.studio-footer{flex-wrap:wrap}.studio-meta{grid-template-columns:2fr 1fr}.studio-meta label:last-child{grid-column:1/-1}.code-studio{padding:20px}.studio-export{flex-wrap:wrap}}
</style>
