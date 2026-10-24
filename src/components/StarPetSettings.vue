<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from 'vue';
import StarPet from './StarPet.vue';
import Icon from './Icon.vue';
import UiSelect from './UiSelect.vue';
import { getStarPetMood, starPetGroups, starPetMoods } from '../starpet';
import type { StarPetMoodId } from '../starpet';
import { getStarPetPreferences, onStarPetChanged, setStarPetPreferences, starPetDesktopAvailable } from '../starpet-api';
import type { StarPetPreferences } from '../starpet-api';

const emit = defineEmits<{ busy: [value: boolean] }>();
const props = defineProps<{ disabled?: boolean }>();
const preferences = ref<StarPetPreferences>({ enabled: false, mood: 'idle' });
const selected = ref<StarPetMoodId>('idle');
const group = ref('all');
const playing = ref(false);
const loading = ref(true);
const busy = ref(false);
const blocked = computed(() => props.disabled || loading.value || busy.value);
const reaction = ref(0);
const error = ref('');
const notice = ref('');
const focusedTile = ref<StarPetMoodId | null>(null);
const mood = computed(() => getStarPetMood(selected.value));
const choices = computed(() => [{ value: 'all', label: '全部表情 · 32' }, ...starPetGroups.map(item => ({ value: item.id, label: item.name }))]);
const shown = computed(() => starPetMoods.filter(item => group.value === 'all' || item.group === group.value));
let timer: ReturnType<typeof setInterval> | undefined;
let unsubscribe: (() => void) | undefined;
let disposed = false, preferenceRevision = 0;
watch(playing, value => {
  clearInterval(timer);
  if (value) timer = setInterval(() => {
    const list = shown.value;
    selected.value = list[(list.findIndex(item => item.id === selected.value) + 1) % list.length]!.id;
  }, 2600);
});
watch(group, () => { if (!shown.value.some(item => item.id === selected.value)) selected.value = shown.value[0]!.id; });
onMounted(async () => {
  try {
    unsubscribe = await onStarPetChanged(value => { preferenceRevision++; preferences.value = value; notice.value = ''; error.value = ''; });
    if (disposed) { unsubscribe(); return; }
    const revision = preferenceRevision;
    const value = await getStarPetPreferences();
    if (!disposed) {
      if (revision === preferenceRevision) preferences.value = value;
      selected.value = preferences.value.mood;
    }
  } catch (reason) { if (!disposed) error.value = String(reason); }
  finally { if (!disposed) loading.value = false; }
});
onBeforeUnmount(() => { disposed = true; unsubscribe?.(); clearInterval(timer); emit('busy', false); });
function select(id: StarPetMoodId) { playing.value = false; selected.value = id; notice.value = ''; }
async function save(enabled: boolean) {
  if (blocked.value || !starPetDesktopAvailable) return;
  busy.value = true; emit('busy', true); error.value = ''; notice.value = ''; playing.value = false;
  try {
    const revision = preferenceRevision;
    const value = await setStarPetPreferences({ enabled, mood: enabled ? selected.value : preferences.value.mood });
    if (disposed) return;
    if (revision === preferenceRevision) preferences.value = value;
    notice.value = preferences.value.enabled ? '星芽已在桌面等你。拖动小把手可以搬家。' : '星芽回去休息了，随时可以再叫出来。';
  } catch (reason) { error.value = String(reason); }
  finally { busy.value = false; emit('busy', false); }
}
</script>

<template>
  <div class="starpet-settings">
    <div class="starpet-intro">
      <div class="starpet-stage"><span class="starpet-orbit" aria-hidden="true" /><button type="button" class="starpet-touch" :disabled="blocked" aria-label="摸摸预览里的星芽" @click="reaction++"><StarPet :mood="selected" :size="168" :reaction="reaction" /></button><span class="starpet-stage-caption">靠近看看，再轻轻点一下</span></div>
      <div class="starpet-intro-copy"><span class="starpet-eyebrow">MEET YOUR LITTLE STAR</span><h3>星伴 <span>· 星芽</span></h3><p>住在桌面的一角，眨眨眼，陪你慢慢收集灵感。</p><strong>{{ mood.name }}</strong><p class="starpet-description">{{ mood.description }}</p>
        <button type="button" class="button primary" :disabled="blocked || !starPetDesktopAvailable" @click="save(!preferences.enabled)"><Icon :name="preferences.enabled ? 'moon' : 'star'" :size="15" />{{ busy ? '正在设置…' : preferences.enabled ? '收起桌面星伴' : '让星芽住在桌面' }}</button>
        <button v-if="preferences.enabled && selected !== preferences.mood" type="button" class="starpet-apply" :disabled="blocked" @click="save(true)">用这个表情陪我 <span aria-hidden="true">↗</span></button>
      </div>
    </div>
    <p class="starpet-status" role="status">{{ error || notice || (starPetDesktopAvailable ? '这里的开关会立即保存。桌面小把手可拖动，点点星芽会回应你。' : '此处可以预览全部表情；桌面悬浮需要在星笺桌面版开启。') }}</p>
    <div class="starpet-gallery-heading"><div><h4>星芽的 32 种小心情</h4><p>点选预览，找到此刻的心情</p></div><button type="button" class="starpet-tour" :disabled="blocked" :aria-pressed="playing" @click="playing = !playing"><Icon :name="playing ? 'pause' : 'play'" :size="13" />{{ playing ? '暂停巡游' : '表情巡游' }}</button></div>
    <UiSelect v-model="group" :disabled="blocked" :options="choices" label="星伴表情分类" />
    <div class="starpet-moods" role="group" aria-label="星芽表情">
      <button v-for="item in shown" :key="item.id" type="button" :disabled="blocked" :class="{ chosen: selected === item.id }" :aria-pressed="selected === item.id" :aria-label="`预览${item.name}`" @click="select(item.id)" @mouseenter="focusedTile = item.id" @mouseleave="focusedTile = null" @focus="focusedTile = item.id" @blur="focusedTile = null"><StarPet :mood="item.id" :size="77" :animated="focusedTile === item.id" /><span>{{ item.name }}</span><Icon v-if="selected === item.id" name="check" :size="11" /></button>
    </div>
    <p class="starpet-footnote">原创矢量角色 · 全部本地运行 · 支持系统减少动态效果<br />星芽目前表达表情和轻互动，不会读取你的剪贴内容，也不需要 AI 服务。</p>
  </div>
</template>

<style scoped>
.starpet-touch{display:flex;align-items:center;justify-content:center;position:relative;border:0;padding:0;border-radius:50%;background:transparent;cursor:pointer;touch-action:manipulation;max-width:100%}
.starpet-settings{padding:2px 0 8px}.starpet-intro{display:grid;grid-template-columns:190px minmax(0,1fr);gap:22px;align-items:center}.starpet-stage{position:relative;display:flex;flex-direction:column;align-items:center;justify-content:center;min-height:210px;border:1px solid var(--line);border-radius:20px;background:radial-gradient(ellipse at 50% 45%,var(--gold-dim),transparent 72%),var(--bg);overflow:hidden}.starpet-orbit{position:absolute;width:152px;height:92px;border:1px dashed var(--line);border-radius:50%;transform:rotate(-28deg);pointer-events:none}.starpet-stage-caption{color:var(--faint);font-size:9px;letter-spacing:1px;margin-top:-6px;padding-bottom:12px}.starpet-eyebrow{font-size:8px;letter-spacing:2px;color:var(--faint)}.starpet-intro h3{font-weight:500;font-size:24px;margin:9px 0}.starpet-intro h3 span{color:var(--gold)}.starpet-intro p{font-size:11px;color:var(--muted);line-height:1.8;margin:0 0 11px}.starpet-intro strong{font-size:12px;color:var(--gold);font-weight:500}.starpet-intro .starpet-description{font-size:10px;margin:5px 0 12px;min-height:18px}.starpet-intro .button{font-size:10px;padding:10px 12px}.starpet-apply{display:block;color:var(--gold);font-size:10px;margin-top:10px;padding:3px 0}.starpet-status{font-size:10px;color:var(--muted);line-height:1.8;padding:12px 0 16px;margin:0;border-bottom:1px solid var(--line);min-height:18px}.starpet-gallery-heading{display:flex;align-items:center;justify-content:space-between;margin:18px 0 12px;gap:12px}.starpet-gallery-heading h4{font-size:12px;font-weight:500;margin:0 0 4px}.starpet-gallery-heading p{font-size:10px;color:var(--faint);margin:0}.starpet-tour{display:flex;align-items:center;gap:6px;color:var(--gold);font-size:10px;white-space:nowrap}.starpet-settings>.ui-select{width:170px}.starpet-moods{display:grid;grid-template-columns:repeat(4,minmax(0,1fr));gap:8px;margin-top:12px}.starpet-moods>button{display:flex;position:relative;flex-direction:column;align-items:center;gap:1px;border:1px solid var(--line);border-radius:12px;background:var(--input);color:var(--muted);padding:5px 4px 11px;font-size:10px;cursor:pointer}.starpet-moods>button:hover{background:var(--hover)}.starpet-moods>button.chosen{border-color:var(--gold);background:var(--gold-dim);color:var(--gold)}.starpet-moods>button>svg{position:absolute;top:7px;right:7px}.starpet-footnote{color:var(--faint);font-size:9px;line-height:1.9;margin:17px 0 0}.starpet-settings :focus-visible{outline:2px solid var(--gold);outline-offset:3px}.starpet-settings button:disabled{opacity:.45;cursor:default}@media(max-width:600px){.starpet-intro{grid-template-columns:135px minmax(0,1fr);gap:14px}.starpet-stage{min-height:186px}.starpet-intro h3{font-size:21px}.starpet-eyebrow{letter-spacing:1px;font-size:7px}.starpet-moods{grid-template-columns:repeat(3,minmax(0,1fr))}}
</style>
