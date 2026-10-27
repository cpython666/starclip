<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import type { Clip, Group } from '../types'
import { aiApi } from '../ai-api'
import type { AiConfig, AiRequest, AiState } from '../ai-api'
import { pendingClip, protectedClip } from '../privacy'
import UiSelect from './UiSelect.vue'
const props = defineProps<{ clips: Clip[]; groups: Group[]; revision: number; disabled?: boolean }>()
const emit = defineEmits<{ busy: [value: boolean] }>()
const state = ref<AiState | null>(null), busy = ref(false), error = ref(''), status = ref('')
const search = ref(''), group = ref<number | null>(null), selected = ref<number[]>([]), name = ref('我的 AI 助手')
const allowPetControl = ref(false)
const fields = ref(['title', 'content']), maxResults = ref(20), expiresHours = ref(24)
const preview = ref<AiRequest | null>(null), config = ref<AiConfig | null>(null), configName = ref('')
const fieldChoices = [{ key: 'title', label: '标题' }, { key: 'content', label: '正文' }, { key: 'tags', label: '普通标签' }, { key: 'sources', label: '来源域名' }]
const groupOptions = computed(() => [{ value: null as number | null, label: '全部分组' }, ...props.groups.map(item => ({ value: item.id, label: item.name }))])
const eligible = computed(() => props.clips.filter(clip => !protectedClip(clip) && !pendingClip(clip, props.revision) && clip.kind !== 'image'))
const available = computed(() => eligible.value.filter(clip => (group.value === null || clip.groupId === group.value) && (!search.value.trim() || `${clip.title}\n${clip.content}\n${clip.tags?.join(' ')}`.toLocaleLowerCase().includes(search.value.trim().toLocaleLowerCase()))))
const chosen = computed(() => eligible.value.filter(clip => selected.value.includes(clip.id)))
const previewClips = computed(() => preview.value ? props.clips.filter(clip => preview.value!.clipIds.includes(clip.id)) : [])
let disposed = false
watch(() => [props.clips, props.revision], () => { selected.value = selected.value.filter(id => eligible.value.some(clip => clip.id === id)); preview.value = null })
watch([name, fields, maxResults, expiresHours, selected, allowPetControl], () => { preview.value = null }, { deep: true })
async function run(action: () => Promise<void>) {
  if (busy.value || props.disabled) return
  busy.value = true; emit('busy', true); error.value = ''; status.value = ''
  try { await action() } catch (cause) { if (!disposed) error.value = String(cause) }
  finally { if (!disposed) { busy.value = false; emit('busy', false) } }
}
async function load() { await run(async () => { const value = await aiApi.state(); if (!disposed) state.value = value }) }
async function toggle() { if (!state.value) return; await run(async () => { const value = await aiApi.enable(!state.value!.enabled); if (!disposed) { state.value = value; config.value = null; preview.value = null; status.value = value.enabled ? '本地接入已开启，请为连接选择资料或星伴权限。' : '已关闭 AI 接入并撤销所有连接。' } }) }
function prepare() { preview.value = { name: name.value.trim(), clipIds: [...selected.value], fields: selected.value.length ? [...fields.value] : [], maxResults: maxResults.value, expiresHours: expiresHours.value, allowPetControl: allowPetControl.value } }
async function create() { if (!preview.value) return; const request = preview.value; await run(async () => { const result = await aiApi.create(request); if (!disposed) { state.value = result.state; config.value = result.config; configName.value = request.name; selected.value = []; allowPetControl.value = false; preview.value = null; status.value = '连接已创建。请将下方配置添加到你信任的本机 AI 客户端。' } }) }
async function revoke(id: string) { await run(async () => { const value = await aiApi.revoke(id); if (!disposed) { state.value = value; config.value = null; status.value = '已撤销连接，后续资料读取与星伴控制都会被拒绝。' } }) }
async function showConfig(id: string, connectionName: string) { await run(async () => { const value = await aiApi.config(id); if (!disposed) { config.value = value; configName.value = connectionName } }) }
async function clearAudit() { await run(async () => { const value = await aiApi.clearAudit(); if (!disposed) state.value = value }) }
function selectVisible() { selected.value = [...new Set([...selected.value, ...available.value.map(clip => clip.id)])].slice(0, state.value?.limits.maxRecords ?? 500) }
onMounted(load)
onBeforeUnmount(() => { disposed = true; config.value = null; emit('busy', false) })
</script>
<template>
  <section class="ai-settings">
    <h3>为 AI 分别授权资料与星伴</h3>
    <p>MCP、CLI 与 Skill 共用本机接口，默认关闭。剪贴历史始终只读；星伴动作与消息需单独授权，也可以只授权星伴而不开放任何记录。隐私、待扫描和图片记录不可授权。</p>
    <p class="field-note">保护范围是星笺官方接口；同账户文件读取或屏幕权限可能绕开它。已交给外部 AI 的内容无法撤回。连接仅本次应用运行有效，重启后需重新授权。</p>
    <p v-if="error" class="privacy-error" role="alert">{{ error }}</p><p v-if="status" class="privacy-status" role="status">{{ status }}</p>
    <button v-if="!state" type="button" class="button subtle" :disabled="busy || disabled" @click="load">{{ busy ? '正在读取接入状态…' : '重新读取接入状态' }}</button>
    <template v-else>
      <div class="setting-row"><div><strong>AI 本机接入</strong><p>{{ !state.available ? '请在桌面版管理真实连接' : state.enabled ? '已开启 · 每个连接单独授权' : '已关闭 · 不接受 AI 调用' }}</p></div><button type="button" :class="['switch', { on: state.enabled }]" role="switch" aria-label="AI 本机接入" :aria-checked="state.enabled" :disabled="busy || disabled || !state.available" @click="toggle"><span /></button></div>
      <div class="ai-connections"><h3>当前连接</h3><p v-if="!state.connections.length" class="field-note">还没有连接。开启接入后，选择具体记录或星伴控制权限即可创建。</p><article v-for="connection in state.connections" :key="connection.id" class="ai-connection"><div><strong>{{ connection.name }}</strong><p>{{ connection.recordCount }} 条记录 · {{ connection.recordCount ? connection.fields.map(field => fieldChoices.find(item => item.key === field)?.label ?? field).join('、') : '不开放剪贴历史' }}<br />星伴动作与消息：{{ connection.allowPetControl ? '已授权' : '未授权' }}<br />到期 {{ new Date(connection.expiresAt).toLocaleString('zh-CN') }}</p></div><div class="privacy-buttons"><button type="button" class="button subtle" :disabled="busy || disabled" @click="showConfig(connection.id, connection.name)">接入配置</button><button type="button" class="button danger-quiet" :disabled="busy || disabled" @click="revoke(connection.id)">撤销</button></div></article></div>
      <fieldset class="ai-create" :disabled="busy || disabled || !state.available || !state.enabled">
        <h3>星伴动作与消息</h3>
        <label class="ai-pet-permission"><input v-model="allowPetControl" type="checkbox" aria-label="允许星伴动作与消息" />允许这个连接控制星伴</label>
        <p class="field-note">Agent 可显示思考、工作、等待、成功或出错状态，触发已解锁的动作，显示最多 120 字的短句。请先在星伴设置中显示星伴；消息最多停留 60 秒，不会改变成长经验或你保存的心情。只用星伴时，无需选择下面的记录或扫描历史。</p>
        <h3>选择本次授权的记录（可选）</h3>
        <p>先筛选再勾选，授权只绑定这批记录的 ID，自动标签和导入不会扩大授权。{{ clips.length - eligible.length }} 条隐私、图片或待扫描记录已排除。</p>
        <div class="ai-filter"><input @keydown.enter.prevent v-model="search" placeholder="筛选标题、正文或标签" aria-label="筛选可授权记录" /><UiSelect v-model="group" :options="groupOptions" label="筛选授权分组" :disabled="busy || disabled || !state.enabled || !state.available" /></div>
        <div class="privacy-buttons"><button type="button" class="button subtle" :disabled="!available.length" @click="selectVisible">选择当前结果</button><button type="button" class="button subtle" :disabled="!selected.length" @click="selected = []">清空选择</button><span>已选 {{ chosen.length }} / {{ state.limits.maxRecords }}</span></div>
        <div class="ai-records"><label v-for="clip in available" :key="clip.id"><input v-model="selected" type="checkbox" :value="clip.id" :disabled="!selected.includes(clip.id) && selected.length >= state.limits.maxRecords" /><span><strong>{{ clip.title || clip.content.slice(0, 70) || '未命名记录' }}</strong><small>{{ clip.tags?.join(' · ') || `记录 #${clip.id}` }}</small></span></label><p v-if="!available.length" class="field-note">{{ allowPetControl ? '没有可授权记录。只授权星伴时可直接预览，无需历史扫描。' : '没有符合条件的记录。可先到「规则与隐私」完成历史扫描。' }}</p></div>
        <label class="privacy-tags-label">连接名称<input @keydown.enter.prevent v-model="name" aria-label="AI 连接名称" maxlength="60" /></label>
        <div class="ai-fields"><label v-for="field in fieldChoices" :key="field.key"><input v-model="fields" type="checkbox" :value="field.key" :disabled="!chosen.length" />{{ field.label }}</label></div>
        <div class="ai-limits"><label>每次资料结果上限<input :disabled="!chosen.length" @keydown.enter.prevent v-model.number="maxResults" type="number" min="1" :max="state.limits.maxResults" /></label><label>有效小时数<input @keydown.enter.prevent v-model.number="expiresHours" type="number" min="1" :max="state.limits.maxHours" /></label></div>
        <button type="button" class="button primary" :disabled="(!chosen.length && !allowPetControl) || (chosen.length > 0 && !fields.length) || !name.trim() || !Number.isInteger(maxResults) || maxResults < 1 || maxResults > state.limits.maxResults || !Number.isInteger(expiresHours) || expiresHours < 1 || expiresHours > state.limits.maxHours" @click="prepare">预览本次授权</button>
        <div v-if="preview" class="ai-preview"><strong>确认向「{{ preview.name }}」{{ preview.clipIds.length ? `开放 ${preview.clipIds.length} 条记录` : '仅授权星伴' }}</strong><p>{{ preview.clipIds.length ? `仅开放 ${preview.fields.map(field => fieldChoices.find(item => item.key === field)?.label).join('、')}` : '不开放剪贴历史' }}，{{ preview.expiresHours }} 小时内有效。{{ preview.clipIds.length ? '提交时后端会再次检查隐私状态。' : '' }}</p><p><strong>星伴动作与消息：{{ preview.allowPetControl ? '允许' : '不允许' }}</strong></p><details v-for="clip in previewClips" :key="clip.id"><summary>记录 #{{ clip.id }}{{ preview.fields.includes('title') ? ` · ${clip.title}` : '' }}</summary><pre v-if="preview.fields.includes('content')">{{ clip.content }}</pre><p v-if="preview.fields.includes('tags')">标签：{{ clip.tags?.join('、') || '无' }}</p><p v-if="preview.fields.includes('sources')">来源：{{ clip.linkHosts?.join('、') || '无' }}</p></details><div class="privacy-buttons"><button type="button" class="button primary" @click="create">确认创建连接</button><button type="button" class="button subtle" @click="preview = null">返回修改</button></div></div>
      </fieldset>
      <div v-if="config" class="ai-config"><div class="privacy-section-heading"><h3>{{ configName }} · 接入配置</h3><button type="button" class="button subtle" @click="config = null">收起凭证</button></div><p>配置包含可撤销凭证，只放入你信任的客户端私有配置；不要分享、截图或提交到 Git。选择文字复制，不会自动修改其他应用。</p><h4>MCP 配置</h4><pre tabindex="0">{{ JSON.stringify(config.mcp, null, 2) }}</pre><h4>CLI</h4><pre tabindex="0">{{ config.cli }}</pre><p class="field-note">授权星伴后，可用 MCP 工具 <code>pet_signal</code> 发送 <code>{"status":"working","message":"正在整理资料","durationSeconds":10}</code>；<code>pet_clear</code> 收起自己发出的提示。CLI 将上方命令末尾的 <code>status</code> 替换成 <code>pet_signal</code> 并传入同样的 JSON 参数。</p><h4>Skill 使用说明</h4><pre tabindex="0">{{ config.skill }}</pre></div>
      <div class="ai-audit"><div class="privacy-section-heading"><h3>本次运行的调用记录</h3><div class="privacy-buttons"><button type="button" class="button subtle" :disabled="busy || disabled" @click="load">刷新</button><button type="button" class="button subtle" :disabled="busy || disabled || !state.audit.length" @click="clearAudit">清空记录</button></div></div><p class="field-note">仅保留最近 200 次操作，不记录搜索词、正文、标题、完整链接或凭证。</p><p v-if="!state.audit.length" class="field-note">暂无调用。</p><ul><li v-for="(entry, index) in state.audit" :key="index">{{ new Date(entry.at).toLocaleTimeString('zh-CN') }} · {{ entry.connectionName }} · {{ entry.action }} · {{ entry.count }} 条 · {{ entry.result }}</li></ul></div>
    </template>
  </section>
</template>
