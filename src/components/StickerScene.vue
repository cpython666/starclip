<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from 'vue'

const props = withDefaults(defineProps<{
  pack: 'none' | 'auto' | 'cosmos' | 'cats' | 'puppies' | 'garden'
  theme: 'dark' | 'light' | 'forest' | 'rose' | 'system'
  placement?: 'sidebar' | 'header' | 'preview'
}>(), { placement: 'sidebar' })

const media = typeof window === 'undefined' ? null : window.matchMedia('(prefers-color-scheme: dark)')
const systemDark = ref(media?.matches ?? true)
const resolvedTheme = computed(() => props.theme === 'system' ? (systemDark.value ? 'dark' : 'light') : props.theme)
const autoPacks = { dark: 'cosmos', light: 'puppies', forest: 'garden', rose: 'cats' } as const
const resolvedPack = computed(() => props.pack === 'auto' ? autoPacks[resolvedTheme.value] : props.pack)
function updateSystemTheme(event: MediaQueryListEvent) { systemDark.value = event.matches }
onMounted(() => media?.addEventListener('change', updateSystemTheme))
onUnmounted(() => media?.removeEventListener('change', updateSystemTheme))
</script>

<template>
  <svg v-if="resolvedPack !== 'none'" class="sticker-scene" :class="`placement-${placement}`" :data-pack="resolvedPack" :data-theme="resolvedTheme" viewBox="0 0 180 90" fill="none" aria-hidden="true" focusable="false">
    <g v-if="resolvedPack === 'cosmos'">
      <path class="trail" d="M27 61c13 17 45 20 76 14m30-38c12-5 23-4 28 1" />
      <g class="paper-piece" transform="rotate(-8 92 43)">
        <path class="cut butter" d="M101 12C83 9 66 21 65 40c-2 19 12 34 31 34 11 0 21-5 27-14-17 7-34-1-36-17-2-13 5-25 14-31Z" />
        <path class="line" d="M101 12C83 9 66 21 65 40c-2 19 12 34 31 34 11 0 21-5 27-14-17 7-34-1-36-17-2-13 5-25 14-31Z" />
        <path class="line" d="m72 36 2 1m7-8 2 1m-8 14q4 3 7-1" />
        <ellipse class="blush" cx="70" cy="43" rx="3.5" ry="2" />
        <path class="cut rose" d="m109 42 6 10 11 2-8 9 1 12-11-5-11 5 2-12-8-9 12-2Z" />
        <path class="line" d="m109 42 6 10 11 2-8 9 1 12-11-5-11 5 2-12-8-9 12-2Z" />
        <path class="line" d="M104 59v1m9-1v1m-8 6q3 2 5-1M88 59q3 8 11 6" />
      </g>
      <g class="paper-piece" transform="rotate(-20 30 28)">
        <circle class="cut lilac" cx="30" cy="28" r="10" />
        <circle class="line" cx="30" cy="28" r="10" />
        <path class="cut no-fill" d="M20 24c-13 1-14 8 10 9s24-7 10-9" />
        <path class="line" d="M20 24c-13 1-14 8 10 9s24-7 10-9" />
        <path class="fine" d="m27 21 4 1" />
      </g>
      <path class="cut mint" d="m148 52 3 7 7 3-7 3-3 7-3-7-7-3 7-3Z" />
      <path class="line" d="m148 52 3 7 7 3-7 3-3 7-3-7-7-3 7-3Z" />
      <path class="spark" d="M132 18v7m-3.5-3.5h7M49 65v5m-2.5-2.5h5" />
      <circle class="dot" cx="157" cy="23" r="1.8" />
    </g>

    <g v-else-if="resolvedPack === 'cats'">
      <path class="trail" d="M28 58c8 10 23 15 37 14m65-45 25-3" />
      <g class="paper-piece" transform="rotate(5 92 45)">
        <path class="cut cream" d="M67 32 67 13 84 24q10-3 20 0l17-11-1 23q8 8 6 20c-1 11-10 16-22 17l-2 8H82l-2-8C66 71 59 64 59 54q-1-13 8-22Z" />
        <path class="line" d="M67 32 67 13 84 24q10-3 20 0l17-11-1 23q8 8 6 20c-1 11-10 16-22 17l-2 8H82l-2-8C66 71 59 64 59 54q-1-13 8-22Z" />
        <path class="rose" d="m72 23 1 10 7-5Zm42 0-7 5 7 5Z" />
        <path class="line" d="M80 46v3m24-3v3m-15 8 3 2 3-2m-3 2v3m0 0q-4 4-7 0m7 0q4 4 7 0M66 53l7 1m-6 5 6-1m38-4 7-1m-7 5 6 1M87 76v4m10-4v4" />
        <ellipse class="blush" cx="76" cy="55" rx="5" ry="2.8" /><ellipse class="blush" cx="109" cy="55" rx="5" ry="2.8" />
        <path class="cut rose" d="M105 25c-10-12-17-6-12 3-6 10 4 15 13 4 11 11 19 3 12-5 5-10-6-14-13-2Z" />
        <path class="line" d="M105 25c-10-12-17-6-12 3-6 10 4 15 13 4 11 11 19 3 12-5 5-10-6-14-13-2Z" />
        <ellipse class="cream outlined" cx="105" cy="29" rx="4" ry="5" />
        <path class="fine" d="m98 27 3 1m8 1 4-1" />
      </g>
      <g class="paper-piece" transform="rotate(-12 31 29)">
        <path class="cut rose" d="M31 41C7 27 23 14 31 25c9-11 24 3 0 16Z" />
        <path class="line" d="M31 41C7 27 23 14 31 25c9-11 24 3 0 16Z" />
      </g>
      <g class="paper-piece" transform="rotate(14 148 61)">
        <circle class="cut lilac" cx="148" cy="61" r="10" /><circle class="line" cx="148" cy="61" r="10" />
        <path class="fine" d="M141 55q10 0 14 9m-16-4q8 0 13 10m-4-18q-3 9 6 16m-2-17q-1 7 6 11M140 68q-12 5-10-2" />
      </g>
      <path class="spark" d="M145 18v6m-3-3h6M42 63v6m-3-3h6" /><circle class="dot" cx="18" cy="57" r="1.7" />
    </g>

    <g v-else-if="resolvedPack === 'puppies'">
      <path class="trail" d="M29 62c10 9 20 12 30 10m79-35 20-8" />
      <g class="paper-piece" transform="rotate(-5 91 46)">
        <path class="cut cream" d="M72 26c-7-12-22-10-25 3-3 12-1 26 9 26 5 0 8-4 10-10l-2 16c-1 12 11 17 28 17s28-5 27-17l-3-16c2 7 5 10 10 10 10 0 13-14 9-26-4-13-19-15-25-3-11-5-27-5-38 0Z" />
        <path class="line" d="M72 26c-7-12-22-10-25 3-3 12-1 26 9 26 5 0 8-4 10-10l-2 16c-1 12 11 17 28 17s28-5 27-17l-3-16c2 7 5 10 10 10 10 0 13-14 9-26-4-13-19-15-25-3-11-5-27-5-38 0Z" />
        <path class="line" d="M72 27q-4 6-6 18m44-18q4 6 6 18M79 45v3m25-3v3" />
        <path class="ink" d="M87 53q5-3 10 0 0 6-5 7-5-1-5-7Z" />
        <path class="rose outlined" d="M88 64v5q4 6 8 0v-5" />
        <path class="line" d="M92 59v3m0 0q-6 7-11 1m11-1q6 7 11 1" />
        <path class="fine" d="m55 29-1 11m72-11 1 11" /><ellipse class="blush" cx="74" cy="56" rx="4" ry="2.4" /><ellipse class="blush" cx="109" cy="56" rx="4" ry="2.4" />
        <path class="cut mint" d="m80 74 12 2 12-2-1 7-11 3-11-3Z" /><path class="fine" d="m80 74 12 2 12-2m-12 2v8" />
        <circle class="butter outlined" cx="92" cy="80" r="3" />
      </g>
      <g class="paper-piece" transform="rotate(-25 28 29)">
        <path class="cut butter" d="M20 23c-6-7-11 2-5 6-6 5 0 13 5 6h16c5 7 11-1 5-6 6-4 1-13-5-6Z" />
        <path class="line" d="M20 23c-6-7-11 2-5 6-6 5 0 13 5 6h16c5 7 11-1 5-6 6-4 1-13-5-6Z" />
      </g>
      <g class="paper-piece" transform="rotate(12 151 63)">
        <path class="cut rose" d="M151 72c-18-10-6-20 0-12 7-8 18 2 0 12Z" /><path class="line" d="M151 72c-18-10-6-20 0-12 7-8 18 2 0 12Z" />
      </g>
      <path class="spark" d="M146 14v7m-3.5-3.5h7M32 59v6m-3-3h6" /><circle class="dot" cx="165" cy="42" r="1.7" />
    </g>

    <g v-else-if="resolvedPack === 'garden'">
      <path class="trail" d="M28 63c12 8 23 11 33 9m72-44 24-8" />
      <g class="paper-piece" transform="rotate(-5 91 47)">
        <path class="cut mint" d="M91 60c-28 1-35-11-31-21 16-4 27 6 31 21Zm3-5c1-20 11-28 25-23 4 12-9 23-25 23Z" />
        <path class="line" d="M91 60c-28 1-35-11-31-21 16-4 27 6 31 21Zm3-5c1-20 11-28 25-23 4 12-9 23-25 23Z" />
        <path class="line" d="M91 66V30m-1 28L69 45m25 9 15-13" />
        <path class="cut rose" d="M91 17c9-12 20-3 13 7 14 4 9 19-3 16-1 15-16 15-20 3-12 7-21-6-10-14-10-10 2-22 12-13 0-9 9-9 8 1Z" />
        <path class="line" d="M91 17c9-12 20-3 13 7 14 4 9 19-3 16-1 15-16 15-20 3-12 7-21-6-10-14-10-10 2-22 12-13 0-9 9-9 8 1Z" />
        <circle class="butter outlined" cx="88" cy="29" r="9" /><path class="fine" d="M84 27v1m8-1v1m-7 5q3 2 5-1" />
        <path class="cut butter" d="m74 62 4 17q13 6 26 0l4-17Z" /><path class="line" d="m74 62 4 17q13 6 26 0l4-17Zm-1 0h36" />
        <path class="fine" d="m84 69 1 7m13-7-1 7" />
      </g>
      <g class="paper-piece" transform="rotate(-17 29 32)">
        <path class="cut lilac" d="M29 30C17 8 4 30 26 36c-18 10 0 20 5 2 7 18 25 5 8-3 22-8 4-25-10-5Z" />
        <path class="line" d="M29 30C17 8 4 30 26 36c-18 10 0 20 5 2 7 18 25 5 8-3 22-8 4-25-10-5Z" /><path class="line" d="m29 30 2 9m-2-9-3-5m3 5 3-5" />
      </g>
      <g class="paper-piece" transform="rotate(12 147 64)">
        <path class="cut mint" d="M145 74c-11-12-7-24 8-26 5 11 2 20-8 26Z" /><path class="line" d="M145 74c-11-12-7-24 8-26 5 11 2 20-8 26Zm0 0 6-18m-4 11-4-6" />
      </g>
      <path class="spark" d="M143 19v6m-3-3h6M44 65v5m-2.5-2.5h5" /><circle class="dot" cx="164" cy="40" r="1.8" />
    </g>
  </svg>
</template>

<style scoped>
.sticker-scene{--paper:#fff9ec;--ink:#665664;--cream:#fff8e9;--butter:#f1d596;--rose:#eeb7c8;--lilac:#bcb5e3;--mint:#a9cfc2;display:block;width:180px;height:90px;max-width:100%;flex-shrink:0;overflow:visible;pointer-events:none;user-select:none}
.sticker-scene[data-theme=light]{--paper:#fff;--ink:#6d655b;--cream:#fffaf0;--butter:#ecd39c;--rose:#e8b6aa;--lilac:#c9c4dc;--mint:#b4cdb8}
.sticker-scene[data-theme=forest]{--paper:#fffef3;--ink:#526653;--cream:#f5f4d9;--butter:#e3d691;--rose:#e3b7aa;--lilac:#c4c5dd;--mint:#a9cba5}
.sticker-scene[data-theme=rose]{--paper:#fffcf7;--ink:#825c6c;--cream:#fff4ec;--butter:#ecd4ab;--rose:#e7b3c4;--lilac:#c7b9dd;--mint:#b8cec0}
.sticker-scene.placement-header{width:110px;height:48px}.sticker-scene.placement-preview{width:100%;height:72px;max-width:180px;margin:auto}.paper-piece{filter:drop-shadow(0 2px 1px #1e192919)}
.cut{stroke:var(--paper);stroke-width:6;stroke-linecap:round;stroke-linejoin:round}.line,.fine,.outlined{stroke:var(--ink);stroke-width:1.6;stroke-linecap:round;stroke-linejoin:round}.fine{stroke-width:1.25}.butter{fill:var(--butter)}.rose{fill:var(--rose)}.lilac{fill:var(--lilac)}.mint{fill:var(--mint)}.cream{fill:var(--cream)}.ink{fill:var(--ink)}.no-fill{fill:none}.blush{fill:var(--rose);opacity:.75}.spark{stroke:var(--butter);stroke-width:2;stroke-linecap:round}.dot{fill:var(--lilac)}.trail{stroke:var(--mint);stroke-width:1.2;stroke-dasharray:2 5;stroke-linecap:round;opacity:.5}
@media(prefers-reduced-motion:reduce){.sticker-scene,.sticker-scene *{animation:none!important;transition:none!important}}
</style>
