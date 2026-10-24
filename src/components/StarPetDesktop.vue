<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from 'vue';
import StarPet from './StarPet.vue';
import Icon from './Icon.vue';
import { getStarPetMood, starPetMoods } from '../starpet';
import type { StarPetMoodId } from '../starpet';
import { dragStarPet, getStarPetPreferences, onStarPetChanged, setStarPetPreferences, showStarPetMainWindow } from '../starpet-api';
import type { StarPetPreferencePatch, StarPetPreferences } from '../starpet-api';

const preferences = ref<StarPetPreferences>({ enabled: false, mood: 'idle', color: '#FFD66E' });
const reaction = ref<StarPetMoodId | null>(null);
const touchCount = ref(0);
const visible = ref(!document.hidden);
const busy = ref(false);
const loading = ref(true);
const error = ref('');
const mood = computed(() => getStarPetMood(reaction.value || preferences.value.mood));
let timer: ReturnType<typeof setTimeout> | undefined;
let unsubscribe: (() => void) | undefined;
let disposed = false, preferenceRevision = 0;
const reactions: StarPetMoodId[] = ['greet', 'happy', 'shy', 'delighted', 'love'];
function visibility() { visible.value = !document.hidden; }
onMounted(async () => {
  document.addEventListener('visibilitychange', visibility);
  try {
    unsubscribe = await onStarPetChanged(value => { preferenceRevision++; preferences.value = value; reaction.value = null; clearTimeout(timer); });
    if (disposed) { unsubscribe(); return; }
    const revision = preferenceRevision;
    const value = await getStarPetPreferences();
    if (!disposed && revision === preferenceRevision) preferences.value = value;
  } catch (reason) { if (!disposed) error.value = String(reason); }
  finally { if (!disposed) loading.value = false; }
});
onBeforeUnmount(() => { disposed = true; clearTimeout(timer); unsubscribe?.(); document.removeEventListener('visibilitychange', visibility); });
function touch() {
  if (loading.value) return;
  reaction.value = reactions[touchCount.value % reactions.length]!;
  touchCount.value++;
  clearTimeout(timer);
  timer = setTimeout(() => { reaction.value = null; }, 2800);
}
async function action(work: () => Promise<unknown>) {
  if (busy.value || loading.value) return;
  busy.value = true; error.value = '';
  try { await work(); } catch (reason) { if (!disposed) error.value = String(reason); }
  finally { if (!disposed) busy.value = false; }
}
async function save(preferencesToSave: StarPetPreferencePatch) {
  const revision = preferenceRevision;
  const value = await setStarPetPreferences(preferencesToSave);
  // A newer tray/window event wins over a delayed command response.
  if (!disposed && revision === preferenceRevision) preferences.value = value;
}
function nextMood() {
  const index = starPetMoods.findIndex(item => item.id === preferences.value.mood);
  return action(() => save({ enabled: true, mood: starPetMoods[(index + 1) % starPetMoods.length]!.id }));
}
function hide() { return action(() => save({ enabled: false })); }
function drag(event: PointerEvent) { if (event.button === 0) void action(dragStarPet); }
function grab(event: PointerEvent) {
  if (event.button !== 0 || busy.value || loading.value) return;
  touch();
  // Native dragging must start on press; macOS may consume release/click events.
  drag(event);
}
</script>

<template>
  <main class="desktop-starpet" aria-label="星伴 · 星芽">
    <button type="button" class="pet-drag" :disabled="busy || loading" aria-label="拖动星芽" title="拖动星芽" @pointerdown.prevent="drag"><span aria-hidden="true">⠿</span></button>
    <button type="button" class="pet-body" :disabled="loading" :aria-label="`摸摸或拖动星芽，现在${mood.name}`" title="轻点摸摸，按住拖动" @pointerdown.prevent="grab" @click="$event.detail === 0 && touch()"><StarPet :mood="mood.id" :color="preferences.color" :size="180" :reaction="touchCount" :animated="preferences.enabled && visible" /></button>
    <span class="pet-caption" role="status">{{ mood.name }}</span>
    <nav class="pet-actions" aria-label="星伴操作"><button type="button" :disabled="busy || loading" title="打开星笺" aria-label="打开星笺" @click="action(showStarPetMainWindow)"><Icon name="grid" :size="14" /></button><button type="button" :disabled="busy || loading" title="换个心情" aria-label="换个心情" @click="nextMood"><Icon name="spark" :size="14" /></button><button type="button" :disabled="busy || loading" title="收起星伴" aria-label="收起星伴" @click="hide"><Icon name="close" :size="14" /></button></nav>
    <p v-if="error" class="pet-error" role="alert">{{ error }}</p>
  </main>
</template>

<style scoped>
.desktop-starpet{width:100vw;height:100vh;display:flex;flex-direction:column;align-items:center;justify-content:center;position:relative;user-select:none;overflow:hidden;font-family:system-ui,-apple-system,sans-serif}.pet-drag{height:18px;width:46px;border-radius:10px;cursor:grab;color:#f3dfae;background:#24232cca;font-size:15px;line-height:16px;opacity:0;transition:opacity .2s}.pet-drag:active,.pet-body:active{cursor:grabbing}.pet-body{display:flex;align-items:center;justify-content:center;width:190px;height:180px;cursor:grab;touch-action:none;background:transparent;border:0;border-radius:50%;padding:0}.pet-caption{font-size:10px;line-height:1.5;padding:3px 10px;border-radius:10px;color:#fff1cc;background:#24232cde;box-shadow:0 2px 8px #0002;max-width:190px;text-align:center}.pet-actions{display:flex;align-items:center;justify-content:center;gap:4px;padding:4px;border-radius:13px;margin-top:5px;background:#24232ceb;opacity:0;transition:opacity .2s}.pet-actions button{display:flex;align-items:center;justify-content:center;width:29px;height:25px;color:#f3dfae;border:0;border-radius:9px;background:transparent;cursor:pointer}.pet-actions button:hover{background:#ffffff1a}.desktop-starpet:hover :is(.pet-drag,.pet-actions),.desktop-starpet:focus-within :is(.pet-drag,.pet-actions){opacity:1}.desktop-starpet :focus-visible{outline:2px solid #f2d793;outline-offset:1px}.pet-actions button:disabled{opacity:.5;cursor:wait}.pet-error{position:absolute;inset:auto 8px 8px;margin:0;background:#352b32;color:#ffe0d4;border:1px solid #d29090;border-radius:8px;padding:7px;font-size:10px;line-height:1.5;max-height:80px;overflow:auto}.pet-body:hover{filter:brightness(1.03)}@media(prefers-reduced-motion:reduce){.pet-drag,.pet-actions{transition:none}}
</style>
