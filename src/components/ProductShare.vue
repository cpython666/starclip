<script setup lang="ts">
import { errorText, t } from '../i18n';
import { computed, onBeforeUnmount, ref, watch } from 'vue';
import Icon from './Icon.vue';
import { copyProductShareImage, copyProductShareText, saveProductShareImage, shareImages, shareImageSize, shareImageSource, shareTexts } from '../product-share';

const props = withDefaults(defineProps<{ disabled?: boolean }>(), { disabled: false });
const emit = defineEmits<{ busy: [value: boolean] }>();
const expanded = ref(false);
const selectedImage = ref(shareImages[0]!.id);
const selectedText = ref(shareTexts[0]!.id);
const image = computed(() => shareImages.find(item => item.id === selectedImage.value)!);
const text = computed(() => shareTexts.find(item => item.id === selectedText.value)!);
const operation = ref<'text' | 'image' | 'save' | null>(null);
const working = computed(() => operation.value !== null);
const canRelease = computed(() => !working.value);
defineExpose({ canRelease });
const blocked = computed(() => props.disabled || working.value);
const status = ref('');
const statusFor = ref<'text' | 'image' | 'save' | null>(null);
const error = ref(false);
let disposed = false;
watch(working, value => emit('busy', value), { flush: 'sync' });

async function run(kind: NonNullable<typeof operation.value>, action: () => Promise<string>) {
  if (disposed || blocked.value) return;
  operation.value = kind; statusFor.value = kind; status.value = ''; error.value = false;
  try { const result = await action(); if (!disposed) status.value = result; }
  catch (reason) { if (!disposed) { error.value = true; status.value = errorText(reason); } }
  finally { if (!disposed) operation.value = null; }
}
function copyText() {
  const id = text.value.id;
  return run('text', async () => { await copyProductShareText(id); return t('文案已复制，可以粘贴给朋友。'); });
}
function copyImage() {
  const { id, label } = image.value;
  return run('image', async () => { await copyProductShareImage(id); return t('「{name}」已复制，可粘贴到支持图片的应用。', { name: t(label) }); });
}
function saveImage() {
  const id = image.value.id;
  return run('save', async () => {
    const result = await saveProductShareImage(id);
    if (result.kind === 'cancelled') return t('已取消保存，没有写入文件。');
    if (result.kind === 'saved') return t('图片已保存：{path}', { path: result.path });
    return t('已发起下载：{name}。请在浏览器下载列表查看。', { name: result.filename });
  });
}
function chooseImage(id: string) { if (!blocked.value) { selectedImage.value = id; status.value = ''; } }
function chooseText(id: string) { if (!blocked.value) { selectedText.value = id; status.value = ''; } }
onBeforeUnmount(() => { disposed = true; emit('busy', false); });
</script>

<template>
  <section class="product-share" :aria-busy="working">
    <button type="button" class="share-entry" :aria-expanded="expanded" aria-controls="product-share-content" :disabled="blocked" @click="expanded = !expanded">
      <span class="share-emblem"><Icon name="spark" :size="23" /></span>
      <span class="share-entry-copy"><strong>{{ t('分享星笺') }}</strong><span>{{ t('文案与产品图片，准备好就能分享') }}</span></span>
      <Icon class="share-entry-arrow" :class="{ expanded }" name="chevron" :size="17" />
    </button>
    <div v-if="expanded" id="product-share-content" class="share-content">
      <div class="share-section-heading"><h3>{{ t('把星笺介绍给朋友') }}</h3><span>{{ t('文案一键复制') }}</span></div>
      <div class="share-text-options" role="group" :aria-label="t('分享文案')">
        <button v-for="item in shareTexts" :key="item.id" type="button" :aria-pressed="selectedText === item.id" :class="{ chosen: selectedText === item.id }" :disabled="blocked" @click="chooseText(item.id)">{{ t(item.label) }}</button>
      </div>
      <p class="share-copy" tabindex="0" :aria-label="t('分享文案内容')">{{ text.content }}</p>
      <button type="button" class="button subtle share-action" :disabled="blocked" @click="copyText"><Icon name="copy" :size="14" />{{ operation === 'text' ? t('正在复制…') : t('复制文案') }}</button>
      <p v-if="status && statusFor === 'text'" :class="['share-status', { error }]" :role="error ? 'alert' : 'status'">{{ status }}</p>

      <div class="share-section-heading share-image-heading"><h3>{{ t('选一张产品介绍图') }}</h3><span>{{ t('已压缩 PNG · {count} 张', { count: shareImages.length }) }}</span></div>
      <div class="share-image-options" role="group" :aria-label="t('产品介绍图')">
        <button v-for="item in shareImages" :key="item.id" type="button" :aria-pressed="selectedImage === item.id" :aria-label="t('选择{name}', { name: t(item.label) })" :class="{ chosen: selectedImage === item.id }" :disabled="blocked" @click="chooseImage(item.id)">
          <img :src="shareImageSource(item)" alt="" :width="item.width" :height="item.height" loading="lazy" decoding="async" />
          <span>{{ t(item.label) }}</span>
        </button>
      </div>
      <figure class="share-preview">
        <img :src="shareImageSource(image)" :alt="t(image.description)" :width="image.width" :height="image.height" decoding="async" />
        <figcaption><span>{{ t(image.label) }}</span><span>{{ image.width }} × {{ image.height }} · {{ shareImageSize(image.bytes) }}</span></figcaption>
      </figure>
      <div class="share-actions">
        <button type="button" class="button primary share-action" :disabled="blocked" @click="copyImage"><Icon name="copy" :size="14" />{{ operation === 'image' ? t('正在复制…') : t('复制图片') }}</button>
        <button type="button" class="button subtle share-action" :disabled="blocked" @click="saveImage"><Icon name="down" :size="14" />{{ operation === 'save' ? t('正在保存…') : t('保存图片') }}</button>
      </div>
      <p v-if="status && statusFor !== 'text'" :class="['share-status', { error }]" :role="error ? 'alert' : 'status'">{{ status }}</p>
      <p class="share-note">{{ t('图片使用原创示例，不包含你的剪贴记录。复制或保存后，由你选择分享到哪里。') }}</p>
    </div>
  </section>
</template>

<style scoped>
.product-share{margin:0 0 18px;border:1px solid var(--line);border-radius:10px;background:var(--input);overflow:hidden;min-width:0}.share-entry{display:flex;align-items:center;gap:12px;width:100%;padding:15px;text-align:left;color:var(--text);cursor:pointer}.share-emblem{display:grid;place-items:center;flex-shrink:0;width:39px;height:39px;border:1px solid var(--line);border-radius:11px;color:var(--gold);background:var(--panel)}.share-entry-copy{display:flex;flex-direction:column;gap:6px;flex:1}.share-entry-copy strong{font-size:13px;font-weight:500}.share-entry-copy>span{font-size:10px;color:var(--muted);line-height:1.6}.share-entry-arrow{flex-shrink:0;color:var(--muted);transition:transform .18s}.share-entry-arrow.expanded{transform:rotate(90deg)}.share-entry:hover{background:var(--panel)}.share-content{border-top:1px solid var(--line);padding:16px}.share-section-heading{display:flex;align-items:center;justify-content:space-between;gap:10px;flex-wrap:wrap;margin-bottom:12px}.share-section-heading h3{font-size:11px;font-weight:500;color:var(--gold);margin:0}.share-section-heading>span{font-size:9px;color:var(--faint)}.share-text-options{display:flex;gap:7px}.share-text-options button{font-size:10px;padding:6px 10px;border:1px solid var(--line);border-radius:5px;color:var(--muted);background:var(--panel);cursor:pointer}.share-text-options button.chosen{border-color:var(--gold);color:var(--gold)}.share-copy{margin:12px 0;font-size:11px;line-height:1.9;color:var(--text);white-space:pre-wrap;overflow-wrap:anywhere;user-select:text;-webkit-user-select:text;max-height:240px;overflow-y:auto}.share-action{font-size:10px;min-height:34px;padding:8px 12px}.share-image-heading{margin-top:23px;padding-top:18px;border-top:1px solid var(--line)}.share-image-options{display:grid;grid-template-columns:repeat(3,minmax(0,1fr));gap:9px}.share-image-options button{overflow:hidden;border:1px solid var(--line);border-radius:6px;background:var(--panel);color:var(--muted);padding:4px;cursor:pointer}.share-image-options button.chosen{border-color:var(--gold);color:var(--gold)}.share-image-options img{width:100%;height:80px;object-fit:cover;object-position:top;border-radius:3px;display:block}.share-image-options span{display:block;margin:8px 0 4px;font-size:10px;line-height:1.5}.share-preview{margin:14px 0 0;min-width:0}.share-preview>img{display:block;width:100%;height:auto;max-height:450px;object-fit:contain;background:var(--panel);border:1px solid var(--line);border-radius:6px}.share-preview figcaption{display:flex;gap:10px;justify-content:space-between;margin-top:8px;font-size:10px;line-height:1.6;color:var(--muted);font-variant-numeric:tabular-nums}.share-actions{display:flex;gap:8px;flex-wrap:wrap;margin-top:13px}.share-status{font-size:10px;line-height:1.8;color:var(--mint);white-space:pre-wrap;overflow-wrap:anywhere;margin:12px 0 0;user-select:text;-webkit-user-select:text}.share-status.error{color:var(--danger)}.share-note{font-size:9px;line-height:1.8;color:var(--faint);margin:12px 0 0}.product-share button:disabled{opacity:.45;cursor:default}.product-share :focus-visible{outline:2px solid var(--gold);outline-offset:3px}.share-entry:focus-visible{outline-offset:-3px}@media(max-width:600px){.share-content{padding:12px}.share-entry{padding:12px}.share-image-options{gap:6px}.share-image-options img{height:64px}.share-actions .button{flex:1}.share-preview figcaption{flex-wrap:wrap}}@media(prefers-reduced-motion:reduce){.share-entry-arrow{transition:none}}
</style>
