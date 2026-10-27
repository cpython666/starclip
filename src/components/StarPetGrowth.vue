<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, useId, watch } from 'vue';
import StarPet from './StarPet.vue';
import { starPetDesktopAvailable } from '../starpet-api';
import { emptyGrowth, growthCatalog, growthSlots, levelForXp, levelProgress, localDay, parseGrowthArchive } from '../starpet-growth';
import type { ActionId, Equipment, GrowthState, Slot, TaskId } from '../starpet-growth';
import { equipStarPetItem, exportStarPetGrowth, getStarPetGrowth, importStarPetGrowth, interactStarPet, onStarPetGrowthChanged, resetStarPetGrowth, setStarPetGrowthEnabled } from '../starpet-growth-api';

const props = withDefaults(defineProps<{ disabled?: boolean; color?: string; mood?: string }>(), { disabled: false, color: '#FFD66E', mood: 'idle' });
const emit = defineEmits<{ change: [value: GrowthState]; action: [id: ActionId]; busy: [value: boolean] }>();
const state = ref<GrowthState>(emptyGrowth());
const loading = ref(true), busy = ref(false), error = ref(''), notice = ref('');
const tab = ref('growth'), slot = ref<Slot | 'all'>('all');
const previewAction = ref<ActionId>('greet'), previewNonce = ref(0);
const tabs = [{ id: 'growth', name: '成长' }, { id: 'outfit', name: '装扮' }, { id: 'achievements', name: '成就' }];
const uid = useId();
const progress = computed(() => levelProgress(state.value.xp));
const today = ref(localDay());
const clockBehind = computed(() => !!state.value.day && today.value < state.value.day);
const completed = computed(() => today.value > state.value.day ? [] : state.value.completedTasks);
const earnedToday = computed(() => growthCatalog.tasks.filter(task => completed.value.includes(task.id)).reduce((total, task) => total + task.xp, 0));
const nextReward = computed(() => [
  ...growthCatalog.items.filter(item => !item.achievement && !state.value.unlockedItems.includes(item.id)).map(item => ({ name: item.name, level: item.level })),
  ...growthCatalog.actions.filter(action => action.level > progress.value.level).map(action => ({ name: action.name, level: action.level })),
].sort((a, b) => a.level - b.level)[0]);
const shownItems = computed(() => growthCatalog.items.filter(item => slot.value === 'all' || item.slot === slot.value));
const blocked = computed(() => props.disabled || loading.value || busy.value);
const pendingImport = ref<{ data: string; state: GrowthState; name: string; revision: number } | null>(null);
const resetRevision = ref<number | null>(null);
const fileInput = ref<HTMLInputElement>();
const importPanel = ref<HTMLElement>(), resetPanel = ref<HTMLElement>();
let unsubscribe: (() => void) | undefined, midnightTimer: ReturnType<typeof setTimeout> | undefined, disposed = false;

function accept(value: GrowthState) {
  if (disposed || value.revision < state.value.revision) return;
  state.value = value;
  emit('change', value);
}
function scheduleDay() {
  clearTimeout(midnightTimer);
  today.value = localDay();
  if (disposed || document.hidden || !state.value.enabled) return;
  const now = new Date(), midnight = new Date(now.getFullYear(), now.getMonth(), now.getDate() + 1);
  midnightTimer = setTimeout(scheduleDay, midnight.getTime() - now.getTime() + 100);
}
watch(() => state.value.enabled, scheduleDay);
async function readState() {
  if (!unsubscribe) {
    unsubscribe = await onStarPetGrowthChanged(accept);
    if (disposed) { unsubscribe(); return; }
  }
  accept(await getStarPetGrowth());
}
onMounted(async () => {
  document.addEventListener('visibilitychange', scheduleDay);
  try {
    await readState();
  } catch (reason) { if (!disposed) error.value = `成长记录未能载入：${String(reason)}`; }
  finally { if (!disposed) loading.value = false; }
});
onBeforeUnmount(() => {
  disposed = true; unsubscribe?.(); clearTimeout(midnightTimer);
  document.removeEventListener('visibilitychange', scheduleDay);
  emit('busy', false);
});
async function run(work: () => Promise<void>) {
  if (blocked.value) return;
  busy.value = true; emit('busy', true); error.value = ''; notice.value = '';
  try { await work(); }
  catch (reason) { if (!disposed) error.value = String(reason); }
  finally { if (!disposed) { busy.value = false; emit('busy', false); } }
}
async function reload() {
  await run(readState);
}
function toggle() {
  return run(async () => {
    const value = await setStarPetGrowthEnabled(!state.value.enabled);
    if (disposed) return;
    accept(value); scheduleDay();
    notice.value = value.enabled ? '成长记录已开启，和星芽打个招呼吧。' : '已暂停成长记录，进度和装扮都保留，仍可与星芽互动。';
  });
}
function interact(id: ActionId, animate = true) {
  return run(async () => {
    const value = await interactStarPet(id);
    if (disposed) return;
    accept(value); scheduleDay();
    previewAction.value = id; previewNonce.value++;
    if (animate) emit('action', id);
    const action = growthCatalog.actions.find(item => item.id === id)!;
    notice.value = `${action.message}。当前 Lv.${levelForXp(state.value.xp)} · 累计 ${state.value.xp} XP。${state.value.enabled ? '每天每项任务只记录一次，想玩还可以再来。' : '开启成长记录后可获得经验。'}`;
    if (clockBehind.value) notice.value = `${action.message}。本机日期早于上次记录，经验暂不增加。`;
  });
}
defineExpose({ greet: () => interact('greet', false) });
function equip(item: typeof growthCatalog.items[number]) {
  return run(async () => {
    const removing = state.value.equipment[item.slot] === item.id;
    accept(await equipStarPetItem(item.slot, removing ? null : item.id));
    if (!disposed) notice.value = `${item.name}已${removing ? '卸下' : '穿戴'}。装扮会自动保存。`;
  });
}
function unequip(slotId: Slot) {
  return run(async () => {
    accept(await equipStarPetItem(slotId, null));
    if (!disposed) notice.value = '装饰已卸下。';
  });
}
function previewEquipment(item: typeof growthCatalog.items[number]): Equipment {
  return { back: null, face: null, hand: null, charm: null, [item.slot]: item.id };
}
function unlockText(item: typeof growthCatalog.items[number]) {
  return item.achievement ? `成就：${growthCatalog.achievements.find(achievement => achievement.id === item.achievement)?.name ?? item.achievement}` : `Lv.${item.level} 解锁`;
}
function taskAction(id: TaskId): ActionId { return id === 'greet' ? 'greet' : id === 'play' ? 'highfive' : 'stretch'; }
function taskActionName(id: TaskId) { return id === 'greet' ? '问个好' : id === 'play' ? '击个掌' : '一起伸懒腰'; }
function itemName(id: string | null) { return growthCatalog.items.find(item => item.id === id)?.name ?? '未穿戴'; }
async function selectTab(id: string) {
  tab.value = id;
  await nextTick();
  document.getElementById(`${uid}-tab-${id}`)?.focus();
}
function tabKey(event: KeyboardEvent, index: number) {
  let next = index;
  if (event.key === 'ArrowRight') next = (index + 1) % tabs.length;
  else if (event.key === 'ArrowLeft') next = (index + tabs.length - 1) % tabs.length;
  else if (event.key === 'Home') next = 0;
  else if (event.key === 'End') next = tabs.length - 1;
  else return;
  event.preventDefault(); void selectTab(tabs[next]!.id);
}
function exportArchive() {
  return run(async () => {
    const path = await exportStarPetGrowth();
    if (!disposed) notice.value = path ? starPetDesktopAvailable ? `成长存档已保存到：${path}` : `已请求浏览器下载：${path}。保存位置由浏览器决定。` : '已取消导出，成长记录未改动。';
  });
}
function chooseImport() { pendingImport.value = null; resetRevision.value = null; fileInput.value?.click(); }
async function readImport(event: Event) {
  const input = event.target as HTMLInputElement, file = input.files?.[0];
  input.value = '';
  if (!file) return;
  await run(async () => {
    if (file.size > 64 * 1024) throw new Error('成长存档不能超过 64 KiB。');
    const data = await file.text(), imported = parseGrowthArchive(data);
    if (disposed) return;
    pendingImport.value = { data, state: imported, name: file.name, revision: state.value.revision };
    await nextTick(); importPanel.value?.focus();
  });
}
function confirmImport() {
  const pending = pendingImport.value;
  if (!pending) return;
  return run(async () => {
    accept(await importStarPetGrowth(pending.data, pending.revision));
    if (!disposed) { pendingImport.value = null; scheduleDay(); notice.value = '成长存档已替换，剪贴历史与星芽外观偏好未改动。'; }
  });
}
async function askReset() {
  pendingImport.value = null; resetRevision.value = state.value.revision;
  await nextTick(); resetPanel.value?.focus();
}
function confirmReset() {
  const revision = resetRevision.value;
  if (revision === null) return;
  return run(async () => {
    accept(await resetStarPetGrowth(revision));
    if (!disposed) { resetRevision.value = null; scheduleDay(); notice.value = '成长记录已重置。剪贴历史、表情、颜色和大小保持原样。'; }
  });
}
</script>

<template>
  <section class="pet-growth" aria-label="星伴养成">
    <div class="growth-heading">
      <div><span class="growth-eyebrow">一起慢慢长大</span><h4>星芽的成长手记</h4><p>离开也没关系，下次见面继续长大。</p></div>
      <button type="button" class="growth-toggle" :aria-pressed="state.enabled" :disabled="blocked" @click="toggle">{{ state.enabled ? '暂停成长记录' : '开启成长记录' }}</button>
    </div>
    <p v-if="!starPetDesktopAvailable" class="growth-demo">浏览器演示 · 成长只在当前页面保留，刷新后重置。</p>
    <div class="growth-level">
      <strong><span>Lv.</span>{{ progress.level }}</strong>
      <div><div class="growth-progress-copy"><span>{{ progress.level === growthCatalog.maxLevel ? '星光长久相伴' : `距离 Lv.${progress.level + 1}` }}</span><span>{{ progress.level === growthCatalog.maxLevel ? `${state.xp} XP · 已满级` : `${progress.current} / ${progress.needed} XP` }}</span></div><progress :value="progress.percent" max="100" :aria-label="`成长等级 ${progress.level}，${progress.current} 点本级经验`" /><p>{{ nextReward ? `下个等级奖励：Lv.${nextReward.level} · ${nextReward.name}` : '等级奖励已全部解锁，日常仍可继续积累经验。' }}</p></div>
    </div>
    <p v-if="!state.enabled" class="growth-note">成长记录尚未开启或已暂停。原有 32 种表情和轻互动始终可用。</p>
    <p v-if="clockBehind" class="growth-note">本机日期早于上次记录，经验暂不增加；已有进度会保留。</p>
    <div class="growth-tabs" role="tablist" aria-label="成长内容">
      <button v-for="(item, index) in tabs" :id="`${uid}-tab-${item.id}`" :key="item.id" type="button" role="tab" :aria-selected="tab === item.id" :aria-controls="`${uid}-panel-${item.id}`" :tabindex="tab === item.id ? 0 : -1" @click="tab = item.id" @keydown="tabKey($event, index)">{{ item.name }}<span v-if="item.id === 'outfit'">{{ state.unlockedItems.length }}</span><span v-if="item.id === 'achievements'">{{ state.achievements.length }}/{{ growthCatalog.achievements.length }}</span></button>
    </div>
    <div v-show="tab === 'growth'" :id="`${uid}-panel-growth`" role="tabpanel" :aria-labelledby="`${uid}-tab-growth`" class="growth-panel">
      <div class="growth-section-title"><h5>今天的小小约定</h5><span>{{ earnedToday }} / 45 XP</span></div>
      <div class="growth-tasks">
        <article v-for="task in growthCatalog.tasks" :key="task.id" :class="{ completed: completed.includes(task.id) }">
          <span class="task-mark" aria-hidden="true">{{ completed.includes(task.id) ? '✓' : '✦' }}</span>
          <div><h6>{{ task.name }} <span>+{{ task.xp }} XP</span></h6><p>{{ task.id === 'create' ? '一起伸懒腰，或在桌面版保存代码、导出卡片' : task.description }}</p></div>
          <span v-if="completed.includes(task.id)" class="task-done">已完成</span><button v-else type="button" :disabled="blocked" @click="interact(taskAction(task.id))">{{ taskActionName(task.id) }}</button>
        </article>
      </div>
      <p class="growth-note">奖励自动收好，每项每天一次，不需要连续打卡。{{ state.interactionDays ? `已经一起度过 ${state.interactionDays} 个有互动的日子。` : '今天就从一个小小的问好开始。' }}</p>
      <div class="growth-section-title"><h5>和星芽玩一会</h5><span>动作结束后恢复原心情</span></div>
      <div class="growth-play"><div class="growth-action-preview" aria-label="星芽动作预览"><StarPet :mood="mood" :color="color" :size="104" :equipment="state.equipment" :action="previewAction" :action-nonce="previewNonce" :animated="tab === 'growth'" /></div><div class="growth-actions"><button v-for="action in growthCatalog.actions" :key="action.id" type="button" :disabled="blocked || action.level > progress.level" @click="interact(action.id)"><span>{{ action.name }}</span><small>{{ action.level > progress.level ? `Lv.${action.level} 解锁` : '一起玩' }}</small></button></div></div>
    </div>
    <div v-show="tab === 'outfit'" :id="`${uid}-panel-outfit`" role="tabpanel" :aria-labelledby="`${uid}-tab-outfit`" class="growth-panel">
      <div class="equipment-slots"><div v-for="item in growthSlots" :key="item.id"><span>{{ item.name }}</span><strong>{{ itemName(state.equipment[item.id]) }}</strong><button v-if="state.equipment[item.id]" type="button" :aria-label="`卸下${item.name}装饰`" :disabled="blocked" @click="unequip(item.id)">卸下</button></div></div>
      <div class="outfit-filters" aria-label="装扮分类"><button type="button" :aria-pressed="slot === 'all'" @click="slot = 'all'">全部</button><button v-for="item in growthSlots" :key="item.id" type="button" :aria-pressed="slot === item.id" @click="slot = item.id">{{ item.name }}</button></div>
      <div class="outfit-grid"><article v-for="item in shownItems" :key="item.id" :class="{ locked: !state.unlockedItems.includes(item.id), wearing: state.equipment[item.slot] === item.id }"><div class="outfit-preview"><StarPet mood="idle" :color="color" :size="96" :equipment="previewEquipment(item)" :animated="false" /></div><h5>{{ item.name }}</h5><p>{{ item.description }}</p><span class="outfit-condition">{{ state.unlockedItems.includes(item.id) ? '已获得' : unlockText(item) }}</span><button type="button" :disabled="blocked || !state.unlockedItems.includes(item.id)" :aria-pressed="state.equipment[item.slot] === item.id" @click="equip(item)">{{ state.equipment[item.slot] === item.id ? '已穿戴 · 卸下' : state.unlockedItems.includes(item.id) ? '穿戴' : '尚未解锁' }}</button></article></div>
      <p class="growth-note">预览不消耗物品，也不增加经验。读书、捧杯等表情会暂时收起冲突的手持玩具，换回其他表情后恢复。</p>
    </div>
    <div v-show="tab === 'achievements'" :id="`${uid}-panel-achievements`" role="tabpanel" :aria-labelledby="`${uid}-tab-achievements`" class="growth-panel">
      <div class="achievement-grid"><article v-for="achievement in growthCatalog.achievements" :key="achievement.id" :class="{ achieved: state.achievements.includes(achievement.id) }"><span class="achievement-badge" aria-hidden="true">{{ state.achievements.includes(achievement.id) ? '✦' : '☆' }}</span><div><h5>{{ achievement.name }}<small>{{ state.achievements.includes(achievement.id) ? '已达成' : '未达成' }}</small></h5><p>{{ achievement.description }}</p><span>奖励 · {{ achievement.reward }}</span></div></article></div>
      <p class="growth-note">成就只获得一次。纪念徽章会留在这里，装饰奖励会放入装扮收藏。</p>
    </div>
    <p v-if="loading || notice || error" class="growth-status" :class="{ error: !!error }" :role="error ? 'alert' : 'status'">{{ loading ? '正在载入成长手记…' : error || notice }} <button v-if="error" type="button" :disabled="blocked" @click="reload">重新读取</button></p>
    <details class="growth-storage">
      <summary>成长存档 · 导入、导出与重置</summary>
      <p>成长存档只包含经验、成就和装扮，不含剪贴内容。现有剪贴历史备份不包含成长，请单独保存。</p>
      <div class="storage-actions"><button type="button" :disabled="blocked" @click="exportArchive">导出成长存档</button><button type="button" :disabled="blocked" @click="chooseImport">导入成长存档</button><button type="button" :disabled="blocked" @click="askReset">重置成长</button></div>
      <input ref="fileInput" type="file" accept=".json,application/json" hidden @change="readImport" />
      <div v-if="pendingImport" ref="importPanel" class="growth-confirm" tabindex="-1" aria-label="确认导入成长存档"><h5>将替换当前成长记录</h5><p class="archive-name">{{ pendingImport.name }}</p><p>存档：Lv.{{ levelForXp(pendingImport.state.xp) }} · {{ pendingImport.state.xp }} XP · {{ pendingImport.state.unlockedItems.length }} 件装饰 · 成长{{ pendingImport.state.enabled ? '开启' : '暂停' }}</p><p>当前：Lv.{{ progress.level }} · {{ state.xp }} XP。导入会替换经验、成就与装扮，不会合并；建议先导出当前存档。</p><p v-if="pendingImport.revision !== state.revision" class="growth-note">当前记录已有变化，请取消后重新选择文件再确认。</p><div class="storage-actions"><button type="button" :disabled="blocked || pendingImport.revision !== state.revision" @click="confirmImport">确认替换成长记录</button><button type="button" :disabled="busy" @click="pendingImport = null">取消</button></div></div>
      <div v-if="resetRevision !== null" ref="resetPanel" class="growth-confirm" tabindex="-1" aria-label="确认重置成长"><h5>重置经验、成就与装扮？</h5><p>当前 {{ state.xp }} XP、{{ state.achievements.length }} 项成就与 {{ state.unlockedItems.length }} 件装饰将被清空，成长记录会暂停。可先导出存档留念；剪贴历史和星芽外观偏好不受影响。</p><p v-if="resetRevision !== state.revision" class="growth-note">当前记录已有变化，请取消后重新发起重置。</p><div class="storage-actions"><button type="button" :disabled="blocked || resetRevision !== state.revision" @click="confirmReset">确认重置成长</button><button type="button" :disabled="busy" @click="resetRevision = null">取消</button></div></div>
    </details>
  </section>
</template>

<style scoped>
.growth-play{display:grid;grid-template-columns:108px minmax(0,1fr);gap:11px;align-items:center}.growth-action-preview{display:flex;align-items:center;justify-content:center;min-height:112px;border-radius:15px;background:radial-gradient(ellipse,var(--gold-dim),transparent 72%)}@media(max-width:600px){.growth-play{grid-template-columns:104px minmax(0,1fr);gap:7px}.growth-play .growth-actions{grid-template-columns:repeat(2,minmax(0,1fr))}}
.pet-growth{margin:16px 0;padding:18px;border:1px solid var(--line);border-radius:18px;background:linear-gradient(140deg,var(--gold-dim),transparent 45%),var(--bg);color:var(--text)}.growth-heading{display:flex;align-items:center;justify-content:space-between;gap:16px}.growth-eyebrow{font-size:9px;letter-spacing:2px;color:var(--gold)}.growth-heading h4{margin:7px 0 5px;font-size:17px;font-weight:500}.growth-heading p,.growth-note{font-size:10px;line-height:1.8;color:var(--muted);margin:4px 0}.growth-toggle{flex-shrink:0;padding:9px 12px;border:1px solid var(--gold);border-radius:10px;color:var(--gold);background:var(--gold-dim);font-size:10px;cursor:pointer}.growth-toggle[aria-pressed=true]{border-color:var(--line);color:var(--muted);background:var(--input)}.growth-demo{padding:8px 10px;border:1px dashed var(--line);border-radius:8px;color:var(--muted);font-size:10px;line-height:1.7}.growth-level{display:flex;align-items:center;gap:17px;margin:20px 0 12px}.growth-level>strong{font-size:34px;color:var(--gold);font-weight:500;white-space:nowrap;font-variant-numeric:tabular-nums}.growth-level>strong>span{font-size:12px;margin-right:3px}.growth-level>div{flex:1;min-width:0}.growth-progress-copy{display:flex;justify-content:space-between;gap:8px;font-size:10px;color:var(--muted)}.growth-level progress{display:block;width:100%;height:6px;margin:8px 0;border:0;border-radius:6px;overflow:hidden;background:var(--line);color:var(--gold);accent-color:var(--gold)}.growth-level progress::-webkit-progress-bar{background:var(--line);border-radius:6px}.growth-level progress::-webkit-progress-value{background:var(--gold);border-radius:6px}.growth-level progress::-moz-progress-bar{background:var(--gold);border-radius:6px}.growth-level p{font-size:10px;color:var(--gold);margin:0;line-height:1.7}.growth-tabs{display:flex;gap:8px;border-bottom:1px solid var(--line);margin-top:17px}.growth-tabs>button{position:relative;padding:10px 13px;color:var(--muted);font-size:11px;cursor:pointer}.growth-tabs>button[aria-selected=true]{color:var(--gold)}.growth-tabs>button[aria-selected=true]:after{content:'';position:absolute;bottom:-1px;left:10px;right:10px;height:2px;background:var(--gold);border-radius:3px}.growth-tabs>button>span{font-size:9px;margin-left:6px;color:var(--faint)}.growth-panel{padding-top:15px}.growth-section-title{display:flex;justify-content:space-between;gap:10px;align-items:center;margin-bottom:10px}.growth-section-title h5{font-size:11px;font-weight:500;margin:0}.growth-section-title>span{font-size:9px;color:var(--faint)}.growth-tasks{display:grid;gap:7px;margin-bottom:8px}.growth-tasks article{display:flex;align-items:center;gap:10px;padding:12px;border:1px solid var(--line);border-radius:10px;background:var(--input)}.task-mark{flex-shrink:0;display:grid;place-items:center;width:23px;height:23px;border-radius:50%;background:var(--gold-dim);color:var(--gold);font-size:12px}.growth-tasks article>div{flex:1;min-width:0}.growth-tasks h6{font-size:11px;font-weight:500;margin:0}.growth-tasks h6>span{font-size:9px;color:var(--gold);margin-left:6px}.growth-tasks p{font-size:9px;color:var(--faint);line-height:1.7;margin:4px 0 0}.growth-tasks button,.task-done{font-size:10px;flex-shrink:0;color:var(--gold)}.growth-tasks button{padding:6px 8px;border:1px solid var(--line);border-radius:7px;cursor:pointer}.growth-tasks .completed .task-mark{background:var(--hover)}.growth-actions{display:grid;grid-template-columns:repeat(3,minmax(0,1fr));gap:7px}.growth-actions>button{display:flex;flex-direction:column;align-items:center;gap:5px;border:1px solid var(--line);border-radius:9px;background:var(--input);padding:10px 6px;color:var(--text);font-size:10px;cursor:pointer}.growth-actions small{font-size:9px;color:var(--faint)}.growth-panel>.growth-note+.growth-section-title{margin-top:20px}.equipment-slots{display:grid;grid-template-columns:repeat(2,minmax(0,1fr));gap:7px}.equipment-slots>div{display:flex;align-items:center;gap:7px;border:1px solid var(--line);border-radius:8px;padding:9px;font-size:9px;background:var(--input)}.equipment-slots span{color:var(--faint);flex-shrink:0}.equipment-slots strong{flex:1;font-weight:400;overflow:hidden;text-overflow:ellipsis;white-space:nowrap}.equipment-slots button{font-size:9px;color:var(--gold);cursor:pointer}.outfit-filters{display:flex;gap:6px;flex-wrap:wrap;margin:13px 0}.outfit-filters button{border:1px solid var(--line);border-radius:12px;padding:5px 10px;font-size:9px;color:var(--muted);cursor:pointer}.outfit-filters button[aria-pressed=true]{border-color:var(--gold);background:var(--gold-dim);color:var(--gold)}.outfit-grid{display:grid;grid-template-columns:repeat(3,minmax(0,1fr));gap:9px}.outfit-grid article{display:flex;flex-direction:column;align-items:center;text-align:center;border:1px solid var(--line);border-radius:12px;background:var(--input);padding:5px 9px 10px}.outfit-grid article.wearing{border-color:var(--gold);background:var(--gold-dim)}.outfit-preview{height:93px;display:flex;align-items:center;justify-content:center}.outfit-grid h5{font-size:10px;font-weight:500;margin:1px 0 5px}.outfit-grid p{font-size:9px;color:var(--faint);margin:0;line-height:1.6;min-height:29px}.outfit-condition{font-size:9px;color:var(--gold);line-height:1.7;margin:7px 0}.outfit-grid button{width:100%;border:1px solid var(--line);border-radius:7px;padding:6px;font-size:9px;color:var(--text);cursor:pointer;margin-top:auto}.outfit-grid .locked .outfit-preview{opacity:.6}.outfit-grid+.growth-note{margin-top:12px}.achievement-grid{display:grid;gap:8px}.achievement-grid article{display:flex;align-items:center;gap:13px;padding:12px;border:1px solid var(--line);border-radius:11px;background:var(--input)}.achievement-badge{display:grid;place-items:center;width:38px;height:38px;border-radius:14px;background:var(--hover);color:var(--faint);font-size:25px;flex-shrink:0}.achieved .achievement-badge{color:var(--gold);background:var(--gold-dim)}.achievement-grid article>div{flex:1}.achievement-grid h5{display:flex;justify-content:space-between;gap:9px;font-weight:500;font-size:11px;margin:0}.achievement-grid h5 small{font-size:9px;color:var(--faint);font-weight:400}.achievement-grid .achieved h5 small{color:var(--gold)}.achievement-grid p{font-size:10px;color:var(--muted);line-height:1.7;margin:5px 0}.achievement-grid article>div>span{font-size:9px;color:var(--faint)}.achievement-grid+.growth-note{margin-top:10px}.growth-status{padding:11px;border:1px solid var(--line);border-radius:9px;color:var(--gold);font-size:10px;line-height:1.8;overflow-wrap:anywhere;background:var(--gold-dim);margin:13px 0 0}.growth-status.error{color:var(--text);background:var(--input)}.growth-status button{font-size:10px;color:var(--gold);text-decoration:underline;padding-left:4px;cursor:pointer}.growth-storage{margin-top:16px;border-top:1px solid var(--line);padding-top:13px}.growth-storage summary{font-size:10px;color:var(--muted);cursor:pointer}.growth-storage p{font-size:10px;line-height:1.8;color:var(--muted)}.storage-actions{display:flex;flex-wrap:wrap;gap:7px}.storage-actions button{padding:7px 10px;border:1px solid var(--line);border-radius:8px;color:var(--text);font-size:10px;background:var(--input);cursor:pointer}.growth-confirm{padding:13px;margin-top:12px;border:1px solid var(--gold);border-radius:10px;background:var(--bg)}.growth-confirm h5{margin:0;font-size:12px;font-weight:500;color:var(--gold)}.archive-name{overflow-wrap:anywhere}.pet-growth button:disabled{opacity:.45;cursor:default}.pet-growth :focus-visible{outline:2px solid var(--gold);outline-offset:3px}.pet-growth button:not(:disabled):hover{background:var(--hover)}@media(max-width:600px){.pet-growth{padding:13px}.growth-heading{gap:9px}.growth-heading h4{font-size:15px}.growth-toggle{padding:8px;font-size:9px}.growth-tasks article{gap:7px;padding:10px 8px}.growth-tasks h6>span{display:block;margin:3px 0 0}.outfit-grid{grid-template-columns:repeat(2,minmax(0,1fr))}.equipment-slots{grid-template-columns:1fr}.growth-tabs>button{padding-left:10px;padding-right:10px}.growth-progress-copy{font-size:9px}.growth-section-title>span{font-size:8px}}
</style>
