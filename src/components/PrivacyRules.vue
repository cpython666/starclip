<script setup lang="ts">
import { errorText, t } from '../i18n';
import { computed, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { api, isDesktop } from '../api'
import type { PrivacyRules, PrivacyScan } from '../privacy'
import UiSelect from './UiSelect.vue'
const props = defineProps<{ disabled?: boolean; revision?: number }>()
const emit = defineEmits<{ busy: [value: boolean]; changed: [] }>()
const saved = ref<PrivacyRules | null>(null), draft = ref<PrivacyRules>({ revision: 0, rules: [] })
const busy = ref(false), error = ref(''), status = ref(''), preview = ref<PrivacyScan | null>(null)
const dirty = computed(() => !!saved.value && JSON.stringify(saved.value) !== JSON.stringify(draft.value))
const canRelease = computed(() => !!saved.value && !busy.value && !dirty.value && !preview.value)
defineExpose({ canRelease })
const conditions = computed(() => [{ value: 'keyword', label: t('包含关键词') }, { value: 'domain', label: t('精确域名') }])
let disposed = false
async function run(action: () => Promise<void>) {
  if (busy.value || props.disabled) return
  busy.value = true; emit('busy', true); error.value = ''; status.value = ''
  try { await action() } catch (cause) { if (!disposed) error.value = errorText(cause) }
  finally { if (!disposed) { busy.value = false; emit('busy', false) } }
}
async function load() { await run(async () => { const value = await api.privacyRules(); if (!disposed) { saved.value = value; draft.value = structuredClone(value); preview.value = null } }) }
async function save() { await run(async () => { const value = await api.savePrivacyRules({ ...draft.value, rules: draft.value.rules.map(rule => ({ ...rule, tag: rule.tag?.trim() || null })) }); if (!disposed) { saved.value = value; draft.value = structuredClone(value); preview.value = null; status.value = t('规则已保存。请预览并扫描已有记录，更新 AI 可用状态。'); emit('changed') } }) }
function add() { draft.value.rules.push({ id: `custom-${Date.now()}-${Math.random().toString(36).slice(2, 8)}`, name: '', enabled: true, condition: 'keyword', value: '', tag: null, markPrivate: true }); preview.value = null }
async function scan() { await run(async () => { const value = await api.previewPrivacyRescan(); if (!disposed) preview.value = value }) }
async function apply() { if (!preview.value) return; const token = preview.value.token; await run(async () => { const result = await api.applyPrivacyRescan(token); if (!disposed) { preview.value = null; status.value = t('已扫描 {total} 条记录，{privateCount} 条受隐私保护。', { total: result.total, privateCount: result.privateCount }); emit('changed') } }) }
watch(() => [props.revision, props.disabled], () => { if (saved.value && props.revision !== saved.value.revision && !dirty.value && !props.disabled && !busy.value) void load() })
onMounted(load)
onBeforeUnmount(() => { disposed = true; emit('busy', false) })
</script>
<template>
  <section class="privacy-settings">
    <h3>{{ t('本地隐私检测与自动标签') }}</h3>
    <p>{{ t('内置检测始终开启：助记词、私钥、疑似凭据和敏感链接。命中记录折叠显示，并阻止星笺官方 AI 接口读取。') }}</p>
    <p class="field-note">{{ t('识别可能误报或漏报；图片尚无 OCR，不向 AI 开放。备份中的规则以停用草稿导入，需手动启用。规则停用不会自动解除已有保护，需在记录详情中复核。数据库和完整备份仍是本机明文。') }}</p>
    <p v-if="!isDesktop" class="field-note">{{ t('浏览器仅演示内存样例与基础规则；完整助记词检测、持久化和 AI 接入在桌面版运行。') }}</p>
    <p v-if="saved && revision !== saved.revision" class="field-note">{{ t('本地规则已更新。当前草稿仍保留，请重新读取最新规则后再编辑。') }}</p>
    <div v-if="error" class="privacy-error" role="alert">{{ error }}</div>
    <p v-if="status" class="privacy-status" role="status">{{ status }}</p>
    <button v-if="!saved" type="button" class="button subtle" :disabled="busy || disabled" @click="load">{{ busy ? t('正在读取规则…') : t('重新读取规则') }}</button>
    <template v-else>
      <fieldset class="privacy-rule-editor" :disabled="busy || disabled">
        <div class="privacy-section-heading"><strong>{{ t('自定义规则') }} <small>{{ draft.rules.length }} / 100</small></strong><button type="button" class="button subtle" :disabled="draft.rules.length >= 100" @click="add">{{ t('添加规则') }}</button></div>
        <p v-if="!draft.rules.length" class="field-note">{{ t('按关键词或精确域名自动添加普通标签、标记隐私。来源域名分类会自动完成，无需创建规则。') }}</p>
        <article v-for="(rule, index) in draft.rules" :key="rule.id" class="privacy-rule">
          <div class="privacy-rule-top"><label><input v-model="rule.enabled" type="checkbox" />{{ t('启用') }}</label><input @keydown.enter.prevent v-model="rule.name" :aria-label="t('规则 {index} 名称', { index: index + 1 })" maxlength="60" :placeholder="t('规则名称，例如：工作资料')" /><button type="button" class="button danger-quiet" :aria-label="t('删除规则 {index}', { index: index + 1 })" @click="draft.rules.splice(index, 1)">{{ t('移除') }}</button></div>
          <div class="privacy-rule-condition"><UiSelect v-model="rule.condition" :options="conditions" :label="t('规则 {index} 条件', { index: index + 1 })" :disabled="busy || disabled" /><input @keydown.enter.prevent v-model="rule.value" :aria-label="t('规则 {index} 匹配内容', { index: index + 1 })" maxlength="200" :placeholder="rule.condition === 'domain' ? t('例如 linux.do；精确匹配，不含子域') : t('输入关键词')" /></div>
          <div class="privacy-rule-actions"><label><input v-model="rule.markPrivate" type="checkbox" />{{ t('标记隐私') }}</label><label>{{ t('普通标签') }} <input @keydown.enter.prevent v-model="rule.tag" :aria-label="t('规则 {index} 普通标签', { index: index + 1 })" maxlength="40" :placeholder="t('选填，例如 工作')" /></label></div>
        </article>
        <div class="privacy-buttons"><button type="button" class="button primary" :disabled="!dirty" @click="save">{{ t('保存规则') }}</button><button type="button" class="button subtle" @click="load">{{ t('重新读取') }}</button><span class="field-note">{{ dirty ? t('有未保存的规则草稿') : t('当前规则版本 {revision}', { revision: saved.revision }) }}</span></div>
      </fieldset>
      <div class="privacy-rescan">
        <h3>{{ t('应用到历史记录') }}</h3><p>{{ t('先查看影响，再确认扫描。只增加保护和标签，不自动解除隐私，也不扩大 AI 授权。') }}</p>
        <button type="button" class="button subtle" :disabled="busy || disabled || dirty" @click="scan">{{ t('预览历史扫描') }}</button><span v-if="dirty" class="field-note">{{ t('请先保存规则，或重新读取放弃草稿。') }}</span>
        <div v-if="preview" class="privacy-scan-preview"><strong>{{ t('本次 {total} 条 · 更新 {changed} 条', { total: preview.total, changed: preview.changedCount }) }}</strong><p>{{ t('扫描后 {total} 条受保护，其中新增隐私 {added} 条。预览不显示敏感正文。', { total: preview.privateCount, added: preview.newlyPrivateCount }) }}</p><div class="privacy-buttons"><button type="button" class="button primary" :disabled="busy || disabled" @click="apply">{{ t('确认扫描历史') }}</button><button type="button" class="button subtle" :disabled="busy || disabled" @click="preview = null">{{ t('取消') }}</button></div></div>
      </div>
    </template>
  </section>
</template>
