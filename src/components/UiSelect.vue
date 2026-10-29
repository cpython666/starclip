<script setup lang="ts" generic="T extends string | number | null">
import { t } from '../i18n';
import { computed, nextTick, onBeforeUnmount, ref, useId, watch } from 'vue'

const props = withDefaults(defineProps<{
  modelValue: T
  options: readonly { value: T; label: string; description?: string }[]
  label: string
  disabled?: boolean
  placeholder?: string
}>(), { disabled: false, placeholder: '' })
const emit = defineEmits<{ 'update:modelValue': [value: T, origin: { x: number; y: number }] }>()
const root = ref<HTMLElement>()
const trigger = ref<HTMLButtonElement>()
const popup = ref<HTMLElement>()
const opened = ref(false)
const active = ref(-1)
const usePopover = ref(typeof HTMLElement !== 'undefined' && typeof HTMLElement.prototype.showPopover === 'function')
const menuId = `ui-select-${useId()}`
const selectedIndex = computed(() => props.options.findIndex(option => Object.is(option.value, props.modelValue)))
const selected = computed(() => props.options[selectedIndex.value])
const activeId = computed(() => opened.value && active.value >= 0 ? `${menuId}-${active.value}` : undefined)
let ownerDialog: HTMLDialogElement | null = null
let observer: MutationObserver | undefined
let resizeObserver: ResizeObserver | undefined
let search = ''
let lastTypedAt = 0

function unavailable() {
  const button = trigger.value
  return props.disabled || !button?.isConnected || button.matches(':disabled') || !button.getClientRects().length
    || !!button.closest('dialog:not([open]), [inert]') || props.options.length === 0
}
function removeListeners() {
  document.removeEventListener('pointerdown', outsidePointer, true)
  document.removeEventListener('focusin', outsideFocus, true)
  document.removeEventListener('scroll', ancestorScroll, true)
  window.removeEventListener('resize', positionMenu)
  window.visualViewport?.removeEventListener('resize', positionMenu)
  window.visualViewport?.removeEventListener('scroll', viewportScroll)
  ownerDialog?.removeEventListener('close', ownerClosed)
  ownerDialog = null
  observer?.disconnect(); observer = undefined
  resizeObserver?.disconnect(); resizeObserver = undefined
}
function close(restoreFocus = false) {
  if (!opened.value) return
  opened.value = false
  removeListeners()
  search = ''; lastTypedAt = 0
  const panel = popup.value
  try {
    if (usePopover.value) panel?.hidePopover()
    else (panel as HTMLDialogElement | undefined)?.close()
  } catch { /* The browser may already have closed a detached or hidden popup. */ }
  if (restoreFocus && trigger.value?.isConnected && !trigger.value.matches(':disabled') && trigger.value.getClientRects().length) trigger.value.focus({ preventScroll: true })
}
function ownerClosed() { close(false) }
function viewportScroll() { close(false) }
function outsidePointer(event: PointerEvent) {
  const panel = popup.value
  // A modal-dialog fallback retargets backdrop clicks to the dialog itself.
  if (event.target === panel && panel) {
    const box = panel.getBoundingClientRect()
    if (event.clientX < box.left || event.clientX > box.right || event.clientY < box.top || event.clientY > box.bottom) { close(true); return }
  }
  if (!root.value?.contains(event.target as Node)) close(false)
}
function outsideFocus(event: FocusEvent) { if (!root.value?.contains(event.target as Node)) close(false) }
function ancestorScroll(event: Event) { if (!popup.value?.contains(event.target as Node)) close(false) }
function positionMenu() {
  const panel = popup.value, button = trigger.value
  if (!opened.value || !panel || !button) return
  if (unavailable()) { close(false); return }
  const viewport = window.visualViewport
  const leftEdge = (viewport?.offsetLeft ?? 0) + 8
  const topEdge = (viewport?.offsetTop ?? 0) + 8
  const rightEdge = leftEdge + (viewport?.width ?? window.innerWidth) - 16
  const bottomEdge = topEdge + (viewport?.height ?? window.innerHeight) - 16
  const box = button.getBoundingClientRect()
  const width = Math.min(Math.max(box.width, 190), Math.max(1, rightEdge - leftEdge))
  panel.style.width = `${width}px`
  panel.style.left = `${Math.max(leftEdge, Math.min(box.left, rightEdge - width))}px`
  const below = Math.max(0, bottomEdge - box.bottom - 6)
  const above = Math.max(0, box.top - topEdge - 6)
  const desired = Math.min(320, panel.scrollHeight || props.options.length * 42 + 12)
  const upward = below < desired && above > below
  panel.style.maxHeight = `${Math.min(320, upward ? above : below)}px`
  const height = panel.getBoundingClientRect().height
  panel.style.top = `${Math.max(topEdge, Math.min(upward ? box.top - height - 6 : box.bottom + 6, bottomEdge - height))}px`
  panel.dataset.side = upward ? 'top' : 'bottom'
}
function addListeners() {
  document.addEventListener('pointerdown', outsidePointer, true)
  document.addEventListener('focusin', outsideFocus, true)
  document.addEventListener('scroll', ancestorScroll, true)
  window.addEventListener('resize', positionMenu)
  window.visualViewport?.addEventListener('resize', positionMenu)
  window.visualViewport?.addEventListener('scroll', viewportScroll)
  ownerDialog = trigger.value?.closest('dialog') ?? null
  ownerDialog?.addEventListener('close', ownerClosed)
  observer = new MutationObserver(() => { if (unavailable()) close(false); else positionMenu() })
  // Observe ancestors only: menu positioning/hover changes must not retrigger this observer.
  for (let node: HTMLElement | null = trigger.value ?? null; node; node = node.parentElement) {
    observer.observe(node, { attributes: true, attributeFilter: ['disabled', 'hidden', 'inert', 'style', 'class', 'open'] })
  }
  if (typeof ResizeObserver !== 'undefined') {
    resizeObserver = new ResizeObserver(positionMenu)
    if (trigger.value) resizeObserver.observe(trigger.value)
  }
}
function reveal(panel: HTMLElement) {
  panel.style.visibility = 'hidden'
  positionMenu()
  if (usePopover.value) panel.showPopover()
  else (panel as HTMLDialogElement).showModal()
  positionMenu()
  panel.style.visibility = ''
}
async function open(index = selectedIndex.value >= 0 ? selectedIndex.value : 0) {
  if (opened.value || unavailable()) return
  active.value = Math.max(0, Math.min(index, props.options.length - 1))
  opened.value = true
  await nextTick()
  if (!opened.value || unavailable() || !popup.value) { close(false); return }
  try { reveal(popup.value) }
  catch {
    if (!usePopover.value) { close(false); return }
    // Native dialogs give older WebViews a top-layer fallback without teleporting
    // outside an enclosing modal (where its contents would become inert).
    usePopover.value = false
    await nextTick()
    if (!opened.value || unavailable() || !popup.value) { close(false); return }
    try { reveal(popup.value) } catch { close(false); return }
  }
  addListeners()
  if (usePopover.value) trigger.value?.focus({ preventScroll: true })
  else popup.value.focus({ preventScroll: true })
  scrollActive()
}
function scrollActive() {
  void nextTick(() => {
    const panel = popup.value, item = document.getElementById(activeId.value ?? '')
    if (!opened.value || !panel || !item) return
    const itemBox = item.getBoundingClientRect(), box = panel.getBoundingClientRect()
    if (itemBox.top < box.top + 6) panel.scrollTop -= box.top + 6 - itemBox.top
    else if (itemBox.bottom > box.bottom - 6) panel.scrollTop += itemBox.bottom - box.bottom + 6
  })
}
function activate(index: number) { active.value = Math.max(0, Math.min(index, props.options.length - 1)); scrollActive() }
function choose(index: number) {
  const option = props.options[index]
  if (unavailable() || !option) return
  const box = trigger.value!.getBoundingClientRect()
  close(true)
  emit('update:modelValue', option.value, { x: box.left + box.width / 2, y: box.top + box.height / 2 })
}
function typeAhead(key: string) {
  const now = Date.now()
  search = now - lastTypedAt > 700 ? key : search + key
  lastTypedAt = now
  const normalized = search.normalize('NFKC').toLocaleLowerCase()
  const repeated = [...normalized].every(char => char === normalized[0])
  const needle = repeated ? normalized[0]! : normalized
  const start = repeated ? active.value + 1 : Math.max(active.value, 0)
  for (let offset = 0; offset < props.options.length; offset++) {
    const index = (start + offset) % props.options.length
    if (props.options[index]!.label.normalize('NFKC').toLocaleLowerCase().startsWith(needle)) { activate(index); return }
  }
}
function keydown(event: KeyboardEvent) {
  if (event.isComposing || event.altKey || event.ctrlKey || event.metaKey || unavailable()) return
  if (event.key === 'Tab') { if (opened.value) close(!usePopover.value); return }
  if (event.key === 'Escape') {
    if (opened.value) { event.preventDefault(); event.stopPropagation(); close(true) }
    return
  }
  if (['ArrowDown', 'ArrowUp', 'Home', 'End', 'Enter', ' '].includes(event.key)) {
    event.preventDefault(); event.stopPropagation()
    if (!opened.value) {
      const initial = event.key === 'Home' ? 0 : event.key === 'End' ? props.options.length - 1 : selectedIndex.value >= 0 ? selectedIndex.value : event.key === 'ArrowUp' ? props.options.length - 1 : 0
      void open(initial)
    } else if (event.key === 'ArrowDown') activate(active.value + 1)
    else if (event.key === 'ArrowUp') activate(active.value - 1)
    else if (event.key === 'Home') activate(0)
    else if (event.key === 'End') activate(props.options.length - 1)
    else choose(active.value)
    return
  }
  if (event.key.length === 1) {
    event.preventDefault(); event.stopPropagation()
    if (!opened.value) void open()
    typeAhead(event.key)
  }
}
function toggle() { if (opened.value) close(true); else void open() }
function popupToggled(event: Event) {
  const panel = popup.value
  if ((event as ToggleEvent).newState !== 'closed' || !panel) return
  const stillOpen = usePopover.value ? panel.matches(':popover-open') : (panel as HTMLDialogElement).open
  if (!stillOpen) close(false)
}
watch(() => props.disabled, disabled => { if (disabled) close(false) })
watch(() => props.options, () => close(false), { deep: true })
watch(() => props.modelValue, () => { if (opened.value) activate(selectedIndex.value >= 0 ? selectedIndex.value : 0) })
onBeforeUnmount(() => { close(false); removeListeners() })
</script>

<template>
  <div ref="root" class="ui-select" :class="{ 'ui-select-open': opened }">
    <button ref="trigger" class="ui-select-trigger" type="button" role="combobox" aria-haspopup="listbox" :aria-label="label" :aria-expanded="opened" :aria-controls="menuId" :aria-activedescendant="activeId" :disabled="disabled || !options.length" @click="toggle" @keydown="keydown">
      <span class="ui-select-value" :class="{ 'ui-select-placeholder': !selected }">{{ selected?.label ?? (placeholder || t('请选择')) }}</span>
      <svg class="ui-select-chevron" width="13" height="13" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" aria-hidden="true"><path d="m4 6 4 4 4-4" stroke-linecap="round" stroke-linejoin="round" /></svg>
    </button>
    <component :is="usePopover ? 'div' : 'dialog'" :id="menuId" ref="popup" class="ui-select-popup" :popover="usePopover ? 'manual' : undefined" :hidden="!opened" role="listbox" :aria-label="label" :aria-activedescendant="activeId" :tabindex="usePopover ? undefined : -1" @keydown="keydown" @click.prevent.stop @toggle="popupToggled" @close.stop="close(false)" @cancel.prevent.stop="close(true)">
      <div v-for="(option, index) in options" :id="`${menuId}-${index}`" :key="index" class="ui-select-option" :class="{ 'ui-select-active': index === active, 'ui-select-selected': index === selectedIndex }" role="option" :aria-selected="index === selectedIndex" @pointermove="active = index" @pointerdown.prevent @click.prevent.stop="choose(index)">
        <span class="ui-select-option-text"><span class="ui-select-option-label">{{ option.label }}</span><span v-if="option.description" class="ui-select-description">{{ option.description }}</span></span>
        <svg v-if="index === selectedIndex" class="ui-select-check" width="15" height="15" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.7" aria-hidden="true"><path d="m3 8 3 3 7-7" stroke-linecap="round" stroke-linejoin="round" /></svg>
      </div>
    </component>
  </div>
</template>

<style scoped>
.ui-select{display:inline-block;min-width:0;max-width:100%;font:inherit;vertical-align:middle}.ui-select .ui-select-trigger{display:flex;align-items:center;justify-content:space-between;gap:10px;width:100%;min-width:0;min-height:32px;margin:0;padding:8px 10px;border:1px solid var(--line);border-radius:8px;background:var(--input);color:var(--text);font:inherit;line-height:1.5;text-align:left;cursor:pointer;box-sizing:border-box;transition:background .15s,border-color .15s}.ui-select .ui-select-trigger:hover:not(:disabled){border-color:var(--muted)}.ui-select.ui-select-open .ui-select-trigger{border-color:var(--gold)}.ui-select .ui-select-trigger:focus-visible{outline:2px solid var(--gold);outline-offset:3px}.ui-select .ui-select-trigger:disabled{cursor:default;opacity:.5}.ui-select-value{overflow:hidden;text-overflow:ellipsis;white-space:nowrap;min-width:0}.ui-select-placeholder{color:var(--muted)}.ui-select-chevron{flex-shrink:0;color:var(--muted);transition:transform .15s}.ui-select-open .ui-select-chevron{transform:rotate(180deg)}
.ui-select-popup{position:fixed;inset:auto;margin:0;padding:5px;box-sizing:border-box;border:1px solid var(--line);border-radius:10px;background:var(--panel);color:var(--text);box-shadow:0 10px 30px #0003,0 2px 7px #0001;max-width:calc(100vw - 16px);max-height:320px;overflow:auto;overscroll-behavior:contain;scrollbar-width:thin;font-family:inherit;font-size:12px;line-height:1.5;outline:none;z-index:1000}.ui-select-popup::backdrop{background:transparent}.ui-select-popup[hidden]{display:none!important}.ui-select-option{display:flex;align-items:center;justify-content:space-between;gap:14px;min-height:34px;padding:8px 10px;border-radius:6px;cursor:pointer;font-size:11px;line-height:1.6;text-align:left}.ui-select-active{background:var(--input)}.ui-select-selected{color:var(--gold)}.ui-select-option-text{display:flex;flex-direction:column;min-width:0;gap:2px}.ui-select-option-label{overflow-wrap:anywhere}.ui-select-description{font-size:10px;line-height:1.6;color:var(--muted);overflow-wrap:anywhere}.ui-select-check{color:var(--gold);flex-shrink:0}.ui-select-popup:focus-visible{outline:2px solid var(--gold);outline-offset:2px}
@media(prefers-reduced-motion:reduce){.ui-select-trigger,.ui-select-chevron{transition:none!important}}
</style>
