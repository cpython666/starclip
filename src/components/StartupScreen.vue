<script setup lang="ts">
import { onMounted, onUnmounted, ref } from 'vue'
import Icon from './Icon.vue'

const emit = defineEmits<{ done: [] }>()
const dialog = ref<HTMLDialogElement>()
const skipButton = ref<HTMLButtonElement>()
const stars = Array.from({ length: 34 }, (_, index) => ({
  left: `${(index * 37 + 9) % 100}%`, top: `${(index * 23 + 7) % 100}%`,
  opacity: 0.2 + (index % 5) * 0.12, width: `${index % 6 === 0 ? 3 : 2}px`,
}))
let timer: ReturnType<typeof setTimeout>
let finished = false
function finish() {
  if (finished) return
  finished = true
  clearTimeout(timer)
  dialog.value?.close()
  emit('done')
}
onMounted(() => {
  dialog.value?.showModal()
  skipButton.value?.focus({ preventScroll: true })
  timer = setTimeout(finish, window.matchMedia('(prefers-reduced-motion: reduce)').matches ? 450 : 1400)
})
onUnmounted(() => { clearTimeout(timer); dialog.value?.close() })
</script>

<template>
  <dialog ref="dialog" class="startup-screen" aria-labelledby="startup-title" aria-describedby="startup-description" @cancel.prevent="finish">
    <div class="startup-stars" aria-hidden="true"><i v-for="(star, index) in stars" :key="index" :style="star" /><span class="startup-meteor meteor-one" /><span class="startup-meteor meteor-two" /><span class="startup-meteor meteor-three" /></div>
    <button ref="skipButton" class="startup-skip" @click="finish">跳过 <Icon name="arrow" :size="15" /></button>
    <div class="startup-center"><div class="startup-emblem" aria-hidden="true"><span /><Icon name="spark" :size="42" /><i>✧</i></div><p class="startup-brand">星笺 <span>STARCLIP</span></p><h1 id="startup-title">拾起每一次<span>灵感</span></h1><p id="startup-description">让散落的灵感，在这里连成星座。</p><div class="startup-rule"><span />YOUR LITTLE UNIVERSE<span /></div></div>
    <p class="startup-caption"><Icon name="shield" :size="13" />只在本机，安心保存</p>
  </dialog>
</template>

<style scoped>
.startup-screen{position:fixed;inset:0;width:100vw;height:100vh;max-width:none;max-height:none;margin:0;padding:0;border:0;overflow:hidden;background:var(--bg);color:var(--text);z-index:1000}.startup-screen::backdrop{background:var(--bg)}.startup-stars{position:absolute;inset:0;pointer-events:none}.startup-stars>i{position:absolute;aspect-ratio:1;background:var(--gold);border-radius:50%}.startup-center{position:absolute;inset:0;display:flex;align-items:center;justify-content:center;flex-direction:column;padding:30px;animation:startup-arrive .6s ease-out both}.startup-emblem{width:114px;height:114px;display:grid;place-items:center;position:relative;margin-bottom:26px;color:var(--gold)}.startup-emblem>span{position:absolute;inset:0;border:1px solid var(--line);border-radius:50%;transform:rotate(-24deg) scaleX(1.4)}.startup-emblem:after{content:'';position:absolute;inset:18px;border:1px dashed var(--line);border-radius:50%}.startup-emblem>i{position:absolute;right:-18px;top:14px;font-size:21px;background:var(--bg);font-style:normal;padding:3px}.startup-brand{font-size:17px;letter-spacing:5px;margin-bottom:22px}.startup-brand>span{font-size:9px;letter-spacing:4px;color:var(--muted);margin-left:13px}.startup-center h1{font-size:42px;font-weight:500;letter-spacing:3px;margin-bottom:16px}.startup-center h1>span{color:var(--gold)}.startup-center>p:last-of-type{font-size:13px;letter-spacing:1px;color:var(--muted)}.startup-rule{display:flex;align-items:center;gap:14px;font-size:8px;letter-spacing:2.5px;color:var(--faint);margin-top:37px}.startup-rule>span{width:30px;height:1px;background:var(--line)}.startup-skip{position:absolute;top:28px;right:30px;display:flex;align-items:center;gap:8px;border:1px solid var(--line);border-radius:20px;color:var(--muted);background:var(--panel);padding:8px 14px;font-size:11px;z-index:2}.startup-skip:hover{color:var(--gold);border-color:var(--gold)}.startup-skip:focus-visible{outline:2px solid var(--gold);outline-offset:4px}.startup-caption{position:absolute;bottom:30px;left:0;right:0;display:flex;align-items:center;justify-content:center;gap:7px;color:var(--faint);font-size:10px;letter-spacing:.5px}.startup-caption svg{color:var(--mint)}.startup-meteor{position:absolute;width:190px;height:1px;border-radius:50%;background:linear-gradient(90deg,var(--gold),transparent);transform:rotate(-35deg);opacity:0;animation:shooting-star 1.05s cubic-bezier(.2,.45,.5,.9) both}.startup-meteor:before{content:'';width:4px;height:4px;position:absolute;left:0;top:-1.5px;border-radius:50%;background:var(--gold);box-shadow:0 0 12px var(--gold)}.meteor-one{left:64%;top:17%;animation-delay:.05s}.meteor-two{left:94%;top:42%;animation-delay:.25s;width:140px;animation-duration:.95s}.meteor-three{left:38%;top:9%;animation-delay:.42s;width:110px;animation-duration:.9s}@keyframes shooting-star{0%{opacity:0;transform:translate(220px,-154px) rotate(-35deg)}15%{opacity:.8}75%{opacity:.6}100%{opacity:0;transform:translate(-600px,420px) rotate(-35deg)}}@keyframes startup-arrive{from{opacity:0;transform:translateY(8px)}to{opacity:1;transform:translateY(0)}}@media(max-width:900px){.startup-center h1{font-size:34px}.startup-emblem{width:96px;height:96px}.startup-skip{top:20px;right:20px}}@media(prefers-reduced-motion:reduce){.startup-center{animation:none}.startup-meteor{display:none}}
</style>
