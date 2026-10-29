<script setup lang="ts">
import { t } from '../i18n';
import { computed, nextTick, onBeforeUnmount, onMounted, ref } from 'vue'
import Icon from './Icon.vue'
import StickerScene from './StickerScene.vue'

const props = withDefaults(defineProps<{
  busy?: boolean
  error?: string
  theme: 'dark' | 'light' | 'white' | 'forest' | 'rose'
}>(), { busy: false, error: '' })
const emit = defineEmits<{ finish: []; dismiss: [] }>()
const dialog = ref<HTMLDialogElement>()
const heading = ref<HTMLElement>()
const step = ref(0)
const steps = [
  {
    label: '拾起灵感', title: '在你准备好时，拾起灵感', pack: 'cosmos', icon: 'pause', badge: '由你决定何时开始',
    intro: '初次打开星笺，剪贴记录默认暂停。先看看这个小小星系，再决定什么时候开始。',
    points: [
      { icon: 'play', title: '亲手开启', text: '准备好后，点击侧栏的「开启剪贴记录」。教程不会替你开启，也不会自动采集内容。' },
      { icon: 'copy', title: '先拾取当前内容', text: '开启或恢复记录时，会先读取一次当前剪贴板中的文本或图片，再记录新的复制；随时可以再次暂停。' },
    ],
    note: '复制密码或其他敏感内容前，记得先暂停记录。',
  },
  {
    label: '留下重要的', title: '给重要的灵感，一个长久的家', pack: 'garden', icon: 'shield', badge: '喜欢的，认真留下',
    intro: '星标、永久保留和分组，各有自己的用处。让常用的内容，下次也能轻松找到。',
    points: [
      { icon: 'star', title: '星标与永久保留', text: '点星星，收入「星标收藏」；点盾牌，标记「永久保留」。两者都会受到清理保护。' },
      { icon: 'folder', title: '用星座整理分组', text: '把相近的内容放在一起。已分组记录不会自动淘汰；手动整理时可选择包含分组。' },
    ],
    note: '想永久保存全部历史？在设置中同时选择「不限条数」和「不限天数」，才会关闭自动淘汰。',
  },
  {
    label: '做成卡片', title: '让灵感，成为一张卡片', pack: 'cats', icon: 'code', badge: '文字也有自己的样子',
    intro: '把常用代码收进「星码集」，或带入「星绘坊」，给代码与文字换一种好看的表达。',
    points: [
      { icon: 'code', title: '星码卡 · 代码有星光', text: '选择语言、背景与排版，预览代码卡片，再导出图片；也可以保存代码或导出 Markdown。' },
      { icon: 'edit', title: '月光笺 · 文字有温度', text: '把正文做成手写纸笺。手写效果需要桌面版和兼容的本机渲染器，按提示连接后使用。' },
    ],
    note: '星绘坊里的两种模式共用正文，切换模式不会替换你的文字。',
  },
  {
    label: '掌管数据', title: '把数据，握在自己手里', pack: 'puppies', icon: 'folder', badge: '自己的灵感，自己保管',
    intro: '内容以明文保存在本机，星笺不会上传云端。备份、迁移和整理，都由你决定。',
    points: [
      { icon: 'down', title: '备份与迁移', text: '桌面版 JSON 备份包含记录、分组与图片数据。完整数据目录迁移会带走已保存偏好等受管数据，并保留旧目录。' },
      { icon: 'spark', title: '星尘整理 · 先看再清理', text: '按大小、时间和类型预览待清理记录，核对后再确认删除；星标与永久保留始终受保护。' },
    ],
    note: '备份文件也是明文，请妥善保管。想再看一遍教程，随时可以回到偏好设置。',
  },
] as const
const current = computed(() => steps[step.value]!)

async function goTo(index: number) {
  if (props.busy || index < 0 || index >= steps.length || index === step.value) return
  step.value = index
  await nextTick()
  heading.value?.focus({ preventScroll: true })
  heading.value?.closest('.onboarding-body')?.scrollTo({ top: 0 })
}
function finish() { if (!props.busy) emit('finish') }
function dismiss() { if (!props.busy && props.error) emit('dismiss') }
function advance() { if (props.error || step.value === steps.length - 1) finish(); else void goTo(step.value + 1) }
onMounted(() => { dialog.value?.showModal(); heading.value?.focus({ preventScroll: true }) })
onBeforeUnmount(() => dialog.value?.close())
</script>

<template>
  <dialog ref="dialog" class="onboarding-guide" aria-labelledby="onboarding-title" :aria-busy="busy" @cancel.prevent="finish">
    <div class="onboarding-shell">
      <header class="onboarding-header">
        <div><p class="onboarding-eyebrow"><Icon name="spark" :size="13" /> A LITTLE HELLO</p><h2 id="onboarding-title">{{ t('初见星笺') }}<span>{{ t('四步，认识你的小小星系') }}</span></h2></div>
        <button type="button" class="onboarding-skip" :disabled="busy" @click="finish">{{ t('跳过教程') }}<Icon name="close" :size="14" /></button>
      </header>
      <nav class="onboarding-progress" :aria-label="t('教程步骤')">
        <ol><li v-for="(item, index) in steps" :key="item.label"><button type="button" :class="{ current: index === step, visited: index < step }" :aria-current="index === step ? 'step' : undefined" :aria-label="t('第 {step} 步：{label}', { step: index + 1, label: t(item.label) })" :disabled="busy" @click="goTo(index)"><span class="onboarding-step-number"><Icon v-if="index < step" name="check" :size="12" /><template v-else>{{ index + 1 }}</template></span><span>{{ t(item.label) }}</span></button></li></ol>
      </nav>
      <div class="onboarding-body">
        <aside class="onboarding-art" aria-hidden="true"><span class="onboarding-orbit" /><StickerScene :pack="current.pack" :theme="theme" placement="preview" /><span class="onboarding-art-label"><Icon :name="current.icon" :size="12" />{{ t(current.badge) }}</span><span class="onboarding-art-stars">✧ · ✦</span></aside>
        <section class="onboarding-copy" :aria-labelledby="`onboarding-step-${step}`">
          <p class="onboarding-counter">{{ String(step + 1).padStart(2, '0') }} <span>/ 04</span></p>
          <h3 :id="`onboarding-step-${step}`" ref="heading" tabindex="-1">{{ t(current.title) }}</h3>
          <p class="onboarding-intro">{{ t(current.intro) }}</p>
          <ul class="onboarding-points"><li v-for="point in current.points" :key="point.title"><span class="onboarding-point-icon"><Icon :name="point.icon" :size="17" /></span><div><h4>{{ t(point.title) }}</h4><p>{{ t(point.text) }}</p></div></li></ul>
          <p class="onboarding-note"><Icon name="shield" :size="14" /><span>{{ t(current.note) }}</span></p>
        </section>
      </div>
      <div v-if="error" class="onboarding-error" role="alert"><Icon name="alert" :size="15" /><div><p>{{ t('暂时没能保存教程状态，请重试。') }}<span>{{ error }}</span></p><button type="button" class="onboarding-dismiss" :disabled="busy" @click="dismiss">{{ t('暂时关闭，下次再提醒') }}</button></div></div>
      <footer class="onboarding-footer">
        <span class="onboarding-footer-note" role="status">{{ busy ? t('正在记下这次相遇…') : t('按自己的节奏，慢慢熟悉。') }}</span>
        <button v-if="step > 0" type="button" class="onboarding-button onboarding-previous" :disabled="busy" @click="goTo(step - 1)"><Icon name="chevron" :size="14" />{{ t('上一步') }}</button>
        <button type="button" class="onboarding-button onboarding-next" :disabled="busy" @click="advance">{{ busy ? t('正在保存…') : error ? t('重试结束教程') : step === steps.length - 1 ? t('完成教程') : t('下一步') }}<Icon :name="step === steps.length - 1 || error ? 'check' : 'arrow'" :size="15" /></button>
      </footer>
    </div>
  </dialog>
</template>

<style scoped>
.onboarding-guide{width:720px;max-width:calc(100vw - 32px);max-height:calc(100vh - 40px);max-height:calc(100dvh - 40px);margin:auto;padding:0;border:1px solid var(--line);border-radius:18px;background:var(--panel);color:var(--text);box-shadow:var(--shadow);overflow:hidden}.onboarding-guide::backdrop{background:var(--backdrop);backdrop-filter:blur(5px)}.onboarding-shell{display:flex;flex-direction:column;max-height:calc(100vh - 42px);max-height:calc(100dvh - 42px)}.onboarding-header{display:flex;justify-content:space-between;align-items:flex-start;gap:16px;padding:25px 28px 22px;flex-shrink:0}.onboarding-eyebrow{display:flex;align-items:center;gap:7px;color:var(--gold);font-size:8px;letter-spacing:2px;margin:0 0 10px}.onboarding-header h2{font-size:24px;font-weight:500;letter-spacing:1px;margin:0;line-height:1.35}.onboarding-header h2>span{display:block;margin-top:7px;font-size:10px;color:var(--muted);letter-spacing:.3px;font-weight:400}.onboarding-skip{display:flex;align-items:center;gap:7px;font-size:10px;color:var(--muted);padding:7px 9px;margin:-2px -7px 0 0;border-radius:6px;background:none;border:0;white-space:nowrap}.onboarding-skip:hover{color:var(--text);background:var(--hover)}
.onboarding-progress{flex-shrink:0;padding:0 28px 17px;border-bottom:1px solid var(--line)}.onboarding-progress ol{display:grid;grid-template-columns:repeat(4,minmax(0,1fr));gap:8px;margin:0;padding:0;list-style:none}.onboarding-progress button{display:flex;align-items:center;gap:7px;width:100%;padding:9px 7px;font-size:10px;color:var(--muted);background:transparent;border:1px solid transparent;border-radius:7px;white-space:nowrap}.onboarding-step-number{display:grid;place-items:center;width:21px;height:21px;border:1px solid var(--line);border-radius:50%;font-size:9px;flex-shrink:0}.onboarding-progress .current{color:var(--gold);background:var(--gold-dim);border-color:var(--line)}.onboarding-progress .current .onboarding-step-number{border-color:var(--gold)}.onboarding-progress .visited .onboarding-step-number{color:var(--mint);border-color:var(--mint)}
.onboarding-body{display:grid;grid-template-columns:150px minmax(0,1fr);gap:27px;padding:25px 28px 28px;overflow:auto;overscroll-behavior:contain;min-height:0}.onboarding-art{align-self:start;display:flex;flex-direction:column;align-items:center;position:relative;padding-top:39px;margin-top:20px;gap:19px;min-width:0}.onboarding-orbit{position:absolute;top:5px;width:130px;height:130px;border:1px dashed var(--line);border-radius:50%;transform:rotate(-16deg) scaleX(.96);pointer-events:none}.onboarding-art-label{display:flex;align-items:center;gap:5px;font-size:9px;color:var(--muted);white-space:nowrap;position:relative;background:var(--panel);padding:3px 0}.onboarding-art-stars{color:var(--gold);font-size:15px;letter-spacing:9px;margin:4px 0 0 9px}.onboarding-counter{font-size:10px;color:var(--gold);letter-spacing:1px;margin:0 0 10px;font-variant-numeric:tabular-nums}.onboarding-counter>span{color:var(--faint);font-size:9px;margin-left:3px}.onboarding-copy h3{font-size:19px;line-height:1.6;font-weight:500;margin:0 0 9px;letter-spacing:.3px;outline:none}.onboarding-intro{font-size:11px;line-height:1.9;color:var(--muted);margin:0}.onboarding-points{list-style:none;margin:21px 0 0;padding:0;display:flex;flex-direction:column;gap:17px}.onboarding-points li{display:flex;align-items:flex-start;gap:10px}.onboarding-point-icon{display:grid;place-items:center;width:29px;height:29px;flex-shrink:0;background:var(--input);color:var(--mint);border:1px solid var(--line);border-radius:8px}.onboarding-points h4{font-size:11px;line-height:1.6;font-weight:500;margin:0 0 4px}.onboarding-points p{font-size:10px;line-height:1.9;color:var(--muted);margin:0}.onboarding-note{display:flex;align-items:flex-start;gap:7px;margin:21px 0 0;padding:11px 12px;border:1px solid var(--line);border-radius:8px;background:var(--input);font-size:10px;line-height:1.8;color:var(--muted)}.onboarding-note svg{margin-top:2px;color:var(--gold)}
.onboarding-error{flex-shrink:0;display:flex;gap:8px;align-items:flex-start;border-top:1px solid var(--line);padding:12px 28px;color:var(--danger);font-size:11px;line-height:1.7;max-height:100px;overflow:auto}.onboarding-error svg{margin-top:2px}.onboarding-error p{margin:0}.onboarding-error p>span{display:block;font-size:10px;overflow-wrap:anywhere}.onboarding-footer{display:flex;align-items:center;justify-content:flex-end;gap:9px;flex-shrink:0;padding:17px 28px;border-top:1px solid var(--line);background:var(--panel)}.onboarding-footer-note{font-size:9px;color:var(--muted);margin-right:auto;line-height:1.7}.onboarding-button{display:inline-flex;align-items:center;justify-content:center;gap:7px;border:1px solid var(--line);border-radius:7px;padding:9px 13px;font-size:11px;line-height:1.5;background:var(--input);color:var(--text);white-space:nowrap}.onboarding-previous{color:var(--muted);background:transparent}.onboarding-previous svg{transform:rotate(180deg)}.onboarding-next{background:var(--gold);border-color:var(--gold);color:var(--on-accent)}.onboarding-guide button:disabled{cursor:wait;opacity:.5}.onboarding-guide button:focus-visible{outline:2px solid var(--gold);outline-offset:3px}.onboarding-guide button{font-family:inherit;cursor:pointer}
@media(max-width:620px){.onboarding-header{padding:20px 20px 17px}.onboarding-progress{padding:0 13px 13px}.onboarding-progress ol{gap:3px}.onboarding-progress button{gap:5px;font-size:9px;padding:7px 5px}.onboarding-step-number{width:19px;height:19px}.onboarding-body{grid-template-columns:minmax(0,1fr);gap:10px;padding:18px 20px 21px}.onboarding-art{display:none}.onboarding-copy h3{font-size:18px}.onboarding-footer{padding:14px 20px}.onboarding-error{padding:11px 20px}.onboarding-footer-note{max-width:100px}.onboarding-button{padding:9px 11px}}
@media(max-height:650px){.onboarding-header{padding-top:17px;padding-bottom:15px}.onboarding-eyebrow{margin-bottom:6px}.onboarding-header h2{font-size:22px}.onboarding-progress{padding-bottom:12px}.onboarding-body{padding-top:20px;padding-bottom:22px}.onboarding-footer{padding-top:13px;padding-bottom:13px}.onboarding-points{gap:13px;margin-top:17px}.onboarding-note{margin-top:17px}}
@media(prefers-reduced-motion:reduce){.onboarding-guide *{animation:none!important;transition:none!important;scroll-behavior:auto!important}}
.onboarding-dismiss{padding:3px 0;margin-top:5px;border:0;background:none;color:var(--text);font-size:10px;line-height:1.7;text-decoration:underline;text-underline-offset:3px}
</style>
