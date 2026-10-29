<script setup lang="ts">
import { computed } from 'vue'
import { formatNumber, t } from '../i18n'
import type { HistorySummary } from '../types'
const props = defineProps<{ history: HistorySummary; disabled?: boolean }>()
defineEmits<{ page: [value: number] }>()
const pages = computed(() => Math.max(1, Math.ceil(props.history.total / props.history.pageSize)))
</script>
<template>
  <nav class="history-pagination" :aria-label="t('历史分页')">
    <div><span>{{ t('第 {page} / {pages} 页 · 共 {count} 条', { page: formatNumber(history.page), pages: formatNumber(pages), count: formatNumber(history.total) }) }}</span><small>{{ t('每页 {count} 条，只加载当前页', { count: history.pageSize }) }}</small></div>
    <div class="history-page-buttons"><button type="button" class="button subtle" :disabled="disabled || history.page <= 1" @click="$emit('page', history.page - 1)">{{ t('上一页') }}</button><button type="button" class="button subtle" :disabled="disabled || history.page >= pages" @click="$emit('page', history.page + 1)">{{ t('下一页') }}</button></div>
  </nav>
</template>
<style scoped>
.history-pagination { display: flex; align-items: center; justify-content: space-between; gap: 12px; margin: 16px 0; color: var(--muted); font-size: 12px; }
.history-pagination small { display: block; margin-top: 4px; font-size: 11px; }
.history-page-buttons { display: flex; flex-shrink: 0; gap: 8px; }
.history-page-buttons .button { min-height: 32px; padding: 6px 12px; }
@media (max-width: 540px) { .history-pagination { align-items: flex-start; flex-wrap: wrap; } }
</style>
