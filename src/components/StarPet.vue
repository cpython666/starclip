<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, useId, watch } from 'vue';
import { getStarPetMood, starPetPalette } from '../starpet';
import { blinkOpenness, starOutline, stepHop, stepSpring } from '../starpet-motion';

const props = withDefaults(defineProps<{ mood?: string; size?: number; animated?: boolean; reaction?: number; color?: string }>(), { mood: 'idle', size: 180, animated: true, reaction: 0, color: '#FFD66E' });
const desired = computed(() => getStarPetMood(props.mood));
const shownMood = ref(desired.value.id);
const expression = computed(() => getStarPetMood(shownMood.value));
// Most moods have no hands. Two small spheres only appear for a gesture or a held prop.
const handPositions = computed<Array<[number, number]>>(() => {
  switch (expression.value.prop) {
    case 'wave': return [[78, 162], [182, 126]];
    case 'flower': return [[78, 162], [156, 171]];
    case 'magnifier': return [[78, 162], [174, 169]];
    case 'book': return [[78, 170], [162, 170]];
    case 'pencil': return [[87, 177], [146, 159]];
    case 'palette': return [[84, 181], [147, 159]];
    case 'mug': return [[99, 177], [146, 177]];
    default: return [];
  }
});
const pixels = computed(() => Number.isFinite(props.size) ? Math.max(48, Math.min(640, props.size)) : 180);
const uid = useId();
const gradientId = `starpet-body-${uid}`;
const glowId = `starpet-glow-${uid}`;
const eyeId = `starpet-eye-${uid}`;
const clipId = `starpet-clip-${uid}`;
const palette = computed(() => starPetPalette(props.color));
const phase = [...uid].reduce((total, char) => total + char.charCodeAt(0), 0) % 23 / 7;
const media = typeof window === 'undefined' ? null : window.matchMedia('(prefers-reduced-motion: reduce)');
const reduced = ref(media?.matches ?? false), visible = ref(true), mounted = ref(false);
const running = computed(() => mounted.value && props.animated && !reduced.value && visible.value);
const still = () => ({ x: 0, y: 0, tilt: desired.value.tilt as number, sx: 1, sy: 1, eyeX: 0, eyeY: 0, faceX: 0, faceY: 0, blink: 1, reveal: 1, tip: 0, side: 0, hand: 0, burst: 0, burstRadius: 50 });
const frame = ref(still());
const bodyTransform = computed(() => `translate(${frame.value.x} ${frame.value.y}) rotate(${frame.value.tilt} 120 124) translate(120 190) scale(${frame.value.sx} ${frame.value.sy}) translate(-120 -190)`);
const faceTransform = computed(() => `translate(${frame.value.faceX} ${frame.value.faceY})`);
const eyeOffset = computed(() => ({ x: frame.value.eyeX + (expression.value.eyes === 'side' ? -2 : 0), y: frame.value.eyeY }));
const outline = computed(() => starOutline(frame.value.tip, frame.value.side));
let gazeX = { value: 0, velocity: 0 }, gazeY = { value: 0, velocity: 0 }, tilt = { value: desired.value.tilt as number, velocity: 0 };
let hop = { x: { value: 0, velocity: 0 }, y: { value: 0, velocity: 0 }, sx: { value: 1, velocity: 0 }, sy: { value: 1, velocity: 0 } };
let pointer = { x: 0, y: 0, inside: false };
let raf = 0, lastStamp = 0, elapsed = 0, hopEpoch = 0, reactionAge = Infinity, transitionAge = Infinity;

function animate(stamp: number) {
  raf = 0;
  if (!running.value) return;
  const dt = lastStamp ? Math.min(.1, (stamp - lastStamp) / 1000) : 0;
  lastStamp = stamp; elapsed += dt; reactionAge += dt; transitionAge += dt;
  if (transitionAge >= .1) shownMood.value = desired.value.id;
  const mode = desired.value.motion, t = elapsed + phase;
  const quiet = mode === 'still' || expression.value.eyes === 'sleep';
  const glancePhase = t % 11;
  const glance = !quiet && glancePhase > 5.2 && glancePhase < 6.7 ? 3.8 : 0;
  gazeX = stepSpring(gazeX, pointer.inside ? pointer.x : glance, dt, 23, .8);
  gazeY = stepSpring(gazeY, pointer.inside ? pointer.y : 0, dt, 21, .8);
  tilt = stepSpring(tilt, desired.value.tilt + gazeX.value * .7, dt, 10, .66);
  let sway = quiet ? .35 * Math.sin(t * .9) : 2.4 * Math.sin(t * 1.15) + .8 * Math.sin(t * 2.13);
  let y = quiet ? .5 * Math.sin(t * 1.7) : -2 - 2.1 * Math.sin(t * 1.7);
  let sx = 1 + .012 * Math.sin(t * 2.1), sy = 1 - .016 * Math.sin(t * 2.1);
  if (mode === 'wave') sway += 3.7 * Math.sin(t * 3.6);
  if (mode === 'tilt') sway += 3 * Math.sin(t * .7);
  if (mode === 'scan') sway += 3.5 * Math.sin(t * 1.8);
  if (mode === 'nod' || mode === 'work') y += 1.7 * Math.sin(t * 3.1);
  if (mode === 'shiver') sway += Math.sin(t * 25) * (t % 3.7 < .75 ? 2.4 : .15);
  if (mode === 'stretch') { const reach = Math.max(0, Math.sin(t * 1.3)); sx -= reach * .06; sy += reach * .1; y -= reach * 3; }
  const hopping = ['bounce', 'hop', 'party'].includes(mode);
  const hopAge = reactionAge < 1.08 ? reactionAge : hopping ? (elapsed - hopEpoch) % (mode === 'party' ? 2.7 : 3.7) - .45 : -1;
  hop = stepHop(hop, hopAge, dt);
  const reveal = transitionAge < .24 ? 1 - .96 * Math.sin(Math.PI * transitionAge / .24) : 1;
  const blink = Math.min(blinkOpenness(elapsed + phase + 1), reactionAge < .3 ? blinkOpenness(Math.max(0, reactionAge - .06)) : 1);
  const burstProgress = (reactionAge - .14) / .81;
  frame.value = {
    x: (quiet ? 0 : Math.sin(t * .83) * 1.6) + hop.x.value, y: y + hop.y.value, tilt: tilt.value + sway,
    sx: sx * hop.sx.value, sy: sy * hop.sy.value,
    eyeX: gazeX.value, eyeY: gazeY.value,
    faceX: gazeX.value * .46 - sway * .16,
    faceY: gazeY.value * .4 + Math.sin(t * 2.1 - .65) * .9 - hop.y.velocity * .014,
    blink, reveal,
    tip: -hop.sy.velocity * 1.7 + Math.sin(t * 2.1 - .6) * 1.5,
    side: -hop.y.velocity * .015 + Math.sin(t * 2.1 - .85) * 1.7,
    hand: -hop.sy.velocity * 5.4 + Math.sin(t * 2.2 - .8) * (mode === 'wave' ? 8 : 4),
    burst: burstProgress > 0 && burstProgress < 1 ? Math.sin(Math.PI * burstProgress) * .9 : 0,
    burstRadius: 50 + Math.max(0, Math.min(1, burstProgress)) * 50,
  };
  raf = requestAnimationFrame(animate);
}
function stop() {
  cancelAnimationFrame(raf); raf = 0; lastStamp = 0; elapsed = hopEpoch = 0; reactionAge = transitionAge = Infinity;
  pointer = { x: 0, y: 0, inside: false };
  gazeX = { value: 0, velocity: 0 }; gazeY = { value: 0, velocity: 0 }; tilt = { value: desired.value.tilt, velocity: 0 };
  hop = { x: { value: 0, velocity: 0 }, y: { value: 0, velocity: 0 }, sx: { value: 1, velocity: 0 }, sy: { value: 1, velocity: 0 } };
  shownMood.value = desired.value.id; frame.value = still();
}
watch(running, active => { stop(); if (active) raf = requestAnimationFrame(animate); }, { flush: 'sync' });
watch(() => desired.value.id, () => {
  if (running.value) transitionAge = 0;
  else { shownMood.value = desired.value.id; frame.value = still(); }
});
watch(() => props.reaction, () => { if (running.value) { reactionAge = 0; hopEpoch = elapsed + 1.08; } });
function visibility() { visible.value = !document.hidden; }
function motionPreference() { reduced.value = media?.matches ?? false; }
onMounted(() => { visibility(); document.addEventListener('visibilitychange', visibility); media?.addEventListener('change', motionPreference); mounted.value = true; });
onBeforeUnmount(() => { mounted.value = false; stop(); document.removeEventListener('visibilitychange', visibility); media?.removeEventListener('change', motionPreference); });
function look(event: PointerEvent) {
  if (!running.value || event.pointerType === 'touch') return;
  const box = (event.currentTarget as HTMLElement).getBoundingClientRect();
  if (!box.width || !box.height) return;
  pointer = { x: Math.max(-5.5, Math.min(5.5, ((event.clientX - box.left) / box.width - .5) * 11)), y: Math.max(-3.5, Math.min(3.5, ((event.clientY - box.top) / box.height - .5) * 7)), inside: true };
}
</script>

<template>
  <div class="starpet" :class="{ 'starpet-animated': running }" :data-mood="expression.id" :data-motion="running ? 'active' : 'still'" :style="{ width: `${pixels}px`, '--starpet-accent': expression.accent }" role="img" :aria-label="`星芽，${expression.name}`" @pointermove="look" @pointerleave="pointer.inside = false">
    <svg class="starpet-canvas" viewBox="0 0 240 240" fill="none" aria-hidden="true" focusable="false">
      <defs>
        <radialGradient :id="gradientId" cx=".35" cy=".27" r=".8"><stop :stop-color="palette.light" /><stop offset=".52" :stop-color="palette.middle" /><stop offset=".83" :stop-color="palette.base" /><stop offset="1" :stop-color="palette.shade" /></radialGradient>
        <radialGradient :id="glowId"><stop stop-color="#fffef1" stop-opacity=".75" /><stop offset="1" stop-color="#fffef1" stop-opacity="0" /></radialGradient>
        <linearGradient :id="eyeId" x1="0" y1="0" x2=".6" y2="1"><stop stop-color="#311e1d" /><stop offset=".55" stop-color="#4c2824" /><stop offset="1" stop-color="#8d4931" /></linearGradient>
        <clipPath :id="clipId"><path :d="outline" /></clipPath>
      </defs>
      <ellipse class="starpet-ground" cx="120" cy="217" :rx="47 * (1 + frame.y * .009)" ry="6" :fill="palette.shade" :opacity=".18 + frame.y * .002" />

      <g v-if="expression.prop === 'orbit'" class="starpet-orbit-decoration"><ellipse cx="120" cy="122" rx="107" ry="45" transform="rotate(-24 120 122)" stroke="var(--starpet-accent)" stroke-width="1.5" stroke-dasharray="2 5" /><g class="starpet-orbiter"><circle cx="218" cy="121" r="8" fill="#9fc8c0" stroke="#557580" stroke-width="1.6" /><path d="m213 119 7 3" stroke="#e9f7e9" stroke-width="2" stroke-linecap="round" /></g></g>
      <g v-if="expression.prop === 'confetti'" class="starpet-confetti" stroke-linecap="round"><path d="m45 38 4 8m145-15-5 8M23 127l8 3m181 21 7 3" stroke="#d996b1" stroke-width="4" /><path d="m25 75 8-3m173-1 8 4M46 179l-5 7m153 3 7-5" stroke="#91bfb2" stroke-width="4" /><path d="m67 29 2 6m100-13-2 6M29 157l5-2m181-45 5-1" stroke="#b1a1d6" stroke-width="3" /><path d="m180 54 2-5 3 5 5 1-4 4v5l-4-3-5 1 2-5-3-4Z" fill="#e4bd71" /></g>
      <g v-if="expression.prop === 'target'" class="starpet-focus-frame" stroke="var(--starpet-accent)" stroke-width="2" stroke-linecap="round"><path d="M51 75V63h12m114 0h12v12M51 170v12h12m114 0h12v-12" /><circle cx="177" cy="63" r="3" fill="var(--starpet-accent)" stroke="none" /></g>

      <g :transform="bodyTransform">
        <g class="starpet-body-motion">
          <path class="starpet-body" :d="outline" :fill="`url(#${gradientId})`" :stroke="palette.base" stroke-width="1.5" stroke-linejoin="round" />
          <g :clip-path="`url(#${clipId})`">
            <ellipse cx="108" cy="113" rx="77" ry="62" :fill="`url(#${glowId})`" opacity=".65" />
            <ellipse cx="68" cy="94" rx="14" ry="5" transform="rotate(-20 68 94)" fill="#fffef1" opacity=".88" />
            <ellipse cx="112" cy="60" rx="11" ry="5" transform="rotate(-48 112 60)" fill="#fffef1" opacity=".9" />
            <ellipse cx="128" cy="47" rx="2.5" ry="2" fill="#fffef1" opacity=".92" />
            <ellipse cx="88" cy="85" rx="2.5" ry="2" fill="#fffef1" opacity=".8" />
          </g>
          <g class="starpet-face" :transform="faceTransform">

          <g :opacity="expression.blush" fill="#ed9f8f"><ellipse cx="82" cy="132" rx="12" ry="8" transform="rotate(-10 82 132)" /><ellipse cx="158" cy="132" rx="12" ry="8" transform="rotate(10 158 132)" /></g>

          <g v-if="expression.eyes === 'droop'" stroke="#7a6967" stroke-width="2.2" stroke-linecap="round"><path d="M87 103q8 0 17-7m32 0q9 7 17 7" /></g>
          <g v-if="expression.id === 'angry'" stroke="#6d5356" stroke-width="2.5" stroke-linecap="round"><path d="m88 101 17 5m30 0 17-5" /></g>
          <g v-for="(x, side) in [98, 142]" :key="x" :transform="`translate(${x} 122) scale(1 ${frame.reveal})`">
            <path v-if="expression.eyes === 'hearts'" d="M0 7C-20-4-5-16 0-7 5-16 20-4 0 7Z" fill="#bd7586" stroke="#9c6473" stroke-width="1" />
            <path v-else-if="expression.eyes === 'stars'" d="m0-12 3.5 7 7.5 1-5.5 5.5 1.3 7.5L0 5.5-6.8 9l1.3-7.5L-11-4l7.5-1Z" fill="#71658d" />
            <path v-else-if="expression.eyes === 'closed'" d="M-8 3Q0-7 8 3" stroke="#57322c" stroke-width="3.4" stroke-linecap="round" />
            <path v-else-if="expression.eyes === 'sleep' || expression.eyes === 'wink' && side === 1" d="M-8-1Q0 7 8-1" stroke="#57322c" stroke-width="3.1" stroke-linecap="round" />
            <path v-else-if="expression.eyes === 'squeezed'" :d="side === 0 ? 'm-7-6 9 6-9 6' : 'm7-6-9 6 9 6'" stroke="#57322c" stroke-width="3" stroke-linecap="round" stroke-linejoin="round" />
            <g v-else class="starpet-blink" :transform="`scale(1 ${frame.blink})`">
              <ellipse v-if="expression.eyes === 'wide'" cy="1" rx="14.5" ry="16" fill="#fff9e9" />
              <g :transform="`translate(${eyeOffset.x} ${eyeOffset.y})`">
                <path v-if="expression.eyes === 'focused'" d="M-7 1h14" stroke="#57322c" stroke-width="4.5" stroke-linecap="round" />
                <g v-else><ellipse :cy="expression.eyes === 'half' || expression.eyes === 'droop' ? 3 : 0" :rx="expression.eyes === 'uneven' && side === 0 ? 5.5 : expression.eyes === 'soft' ? 6.5 : 7.5" :ry="expression.eyes === 'half' ? 5 : expression.eyes === 'soft' ? 9 : expression.eyes === 'droop' || expression.eyes === 'uneven' && side === 0 ? 8 : 12" :fill="`url(#${eyeId})`" /><ellipse v-if="expression.eyes !== 'half'" cx="-2" :cy="expression.eyes === 'droop' ? 1 : -3" rx="2.4" ry="2.9" fill="#fffcef" opacity=".92" /><circle v-if="expression.eyes === 'wide'" cx="2" cy="4" r="1.1" fill="#c5d0dd" /></g>
              </g>
              <path v-if="expression.eyes === 'half'" d="M-8 0h16" stroke="#5a5b69" stroke-width="2.2" stroke-linecap="round" />
            </g>
          </g>

          <g :opacity=".2 + frame.reveal * .8" :transform="`translate(120 141) scale(1.1 ${.82 + frame.reveal * .28}) translate(-120 -141)`" stroke="#63372d" stroke-width="2.3" stroke-linecap="round" stroke-linejoin="round">
            <path v-if="expression.mouth === 'smile'" d="M111 139q9 10 18 0" />
            <g v-else-if="expression.mouth === 'grin'"><path d="M108 137q12 5 24 0c0 19-24 19-24 0Z" fill="#6d5662" /><path d="M113 146q7-4 14 0-7 7-14 0Z" fill="#e4a19d" stroke="none" /></g>
            <path v-else-if="expression.mouth === 'tiny'" d="M116 141q4 4 8 0" />
            <ellipse v-else-if="expression.mouth === 'o'" cx="120" cy="141" rx="5" ry="7" fill="#74616b" stroke="none" />
            <g v-else-if="expression.mouth === 'yawn'"><ellipse cx="120" cy="143" rx="7" ry="10" fill="#75616b" stroke="none" /><ellipse cx="120" cy="148" rx="4" ry="2.4" fill="#dda5a1" stroke="none" /></g>
            <path v-else-if="expression.mouth === 'flat'" d="M115 142h10" />
            <path v-else-if="expression.mouth === 'wave'" d="M110 143q3-6 7 0t7 0 7 0" />
            <path v-else-if="expression.mouth === 'frown'" d="M111 145q9-10 18 0" />
            <path v-else-if="expression.mouth === 'wail'" d="M109 145q11-18 22 0c3 8-25 8-22 0Z" fill="#7f646c" />
            <path v-else-if="expression.mouth === 'smirk'" d="M111 143q10 5 19-6m-2-1 4 1" />
            <g v-else-if="expression.mouth === 'tongue'"><path d="M110 139q10 7 20 0" /><path d="M120 143v5q5 7 8-1v-5" fill="#dfa1a2" /><path d="m124 144 1 4" stroke="#b67786" stroke-width="1" /></g>
            <path v-else-if="expression.mouth === 'breath'" d="M116 139q9 3 0 7" />
          </g>
          </g>
          <g :opacity=".2 + frame.reveal * .8">

          <path v-if="expression.prop === 'wave'" d="m190 109 3-5m5 13 6-2" stroke="var(--starpet-accent)" stroke-width="2.5" stroke-linecap="round" />
          <g v-if="expression.prop === 'question' || expression.prop === 'puzzle'" class="starpet-floating-prop" stroke="var(--starpet-accent)" stroke-linecap="round" stroke-width="4"><path d="M172 49c-1-15 20-17 20-4 0 7-11 8-11 16" /><path d="M181 70v.1" stroke-width="5" /><path v-if="expression.prop === 'puzzle'" d="M52 66c-2-9 11-13 13-5 1 5-6 7-5 12m1 8v.1" stroke-width="3" /></g>
          <g v-if="expression.prop === 'sound'" class="starpet-listen-lines" stroke="var(--starpet-accent)" stroke-width="2.5" stroke-linecap="round"><path d="M49 111q-9 9 0 18m-7-25q-17 16 0 32M184 119q5 4 0 8" /></g>
          <path v-if="expression.prop === 'moon'" class="starpet-floating-prop" d="M64 43c-15-4-27 14-17 26 8 10 22 6 27-4-17 5-27-11-10-22Z" fill="#b3b6d6" stroke="#7e8aa9" stroke-width="1.6" />
          <g v-if="expression.prop === 'zzz'" class="starpet-sleep-marks" stroke="var(--starpet-accent)" stroke-width="2.6" stroke-linecap="round" stroke-linejoin="round"><path d="M157 54h9l-9 10h10M174 36h12l-12 13h13M193 17h14l-14 15h15" /></g>
          <g v-if="expression.prop === 'dawn'" class="starpet-floating-prop" stroke="#c99d62" stroke-width="1.8" stroke-linecap="round"><circle cx="59" cy="51" r="11" fill="#efcd88" /><path d="M59 33v-5m0 46v-5M41 51h-5m46 0h-5M46 38l-3-3m32 32-3-3m0-26 3-3M43 67l3-3" /></g>
          <g v-if="expression.prop === 'stretch'" stroke="var(--starpet-accent)" stroke-width="2" stroke-linecap="round"><path d="M40 75q-7 3-9 12m0-12q-7 5-8 14m177-14q7 3 9 12m0-12q7 5 8 14" /><path d="m94 34-2-7m54 7 2-7" /></g>
          <g v-if="['spark', 'sparkles', 'check'].includes(expression.prop)" class="starpet-sparkles" fill="var(--starpet-accent)"><path d="m53 63 3 7 7 3-7 3-3 7-3-7-7-3 7-3Z" /><path v-if="expression.prop === 'sparkles'" d="m181 40 4 9 9 4-9 4-4 9-4-9-9-4 9-4Zm22 105 2 5 5 2-5 2-2 5-2-5-5-2 5-2Z" /><circle cx="190" cy="83" r="2.5" /></g>
          <g v-if="expression.prop === 'hearts'" class="starpet-hearts" fill="var(--starpet-accent)" stroke="#b77791" stroke-width="1.2"><path d="M181 68c-29-17-9-33 0-20 10-13 30 3 0 20Z" /><path d="M53 66c-18-11-6-22 0-13 7-9 20 2 0 13Z" /><path d="M196 152c-14-8-5-17 0-10 5-7 15 1 0 10Z" /></g>
          <g v-if="expression.prop === 'flower'" transform="rotate(17 163 143)"><path d="m163 158-3 24" stroke="#7d9b82" stroke-width="2.5" stroke-linecap="round" /><path d="M160 175q12-11 16-2-7 7-16 2Z" fill="#9fbea3" /><path d="M162 136c6-10 15-3 9 4 12 2 8 14-1 11 1 12-12 13-13 3-9 6-15-5-7-10-8-8 1-16 8-8-1-9 6-9 4 0Z" fill="#dfb1c2" stroke="#b1839c" stroke-width="1.2" /><circle cx="161" cy="146" r="5" fill="#f9dd8e" /></g>
          <g v-if="expression.prop === 'surprise'" class="starpet-surprise-lines" stroke="var(--starpet-accent)" stroke-width="3" stroke-linecap="round"><path d="m60 53 7 12m-22 3 12 7m118-22-7 12m22 3-12 7M120 17v9" /></g>
          <g v-if="expression.prop === 'cloud'" class="starpet-floating-prop"><path d="M91 44c-8-8 0-19 10-17 0-15 24-17 30-4 13-5 24 6 18 16 8 4 3 12-5 11h-43q-8 0-10-6Z" fill="#b2bfd2" stroke="#899ab5" stroke-width="1.4" /><path d="m104 57-2 5m18-4-2 5m18-5-2 5" stroke="#9caec8" stroke-width="2" stroke-linecap="round" /></g>
          <g v-if="expression.prop === 'tears'" class="starpet-tears" fill="#9fc9dd" stroke="#73a8c6" stroke-width="1"><path d="M96 123c-1 10-8 16-7 24 1 8 10 8 11 0 1-8-3-14-4-24Zm48 0c1 10 8 16 7 24-1 8-10 8-11 0-1-8 3-14 4-24Z" /><circle cx="89" cy="159" r="3" /><circle cx="151" cy="159" r="3" /></g>
          <g v-if="expression.prop === 'anger'" stroke="var(--starpet-accent)" stroke-width="3" stroke-linecap="round"><path d="M164 54q8 0 8-8m6 0q0 8 8 8m0 6q-8 0-8 8m-6 0q0-8-8-8" /><path d="m52 144-6 3m7 4-5 5" stroke-width="2" /></g>
          <g v-if="expression.prop === 'sweat'" class="starpet-floating-prop"><path d="M173 79c-2-9 3-18 7-22 1 8 8 17 5 23-3 6-10 5-12-1Z" fill="#a0ccda" stroke="#76a8be" stroke-width="1.4" /><path d="m58 124-4 9m10-7-3 8" stroke="#b7a6a3" stroke-width="1.6" stroke-linecap="round" /></g>
          <g v-if="expression.prop === 'crown'" transform="rotate(3 122 36)"><path d="m103 43-3-22 14 9 8-17 9 17 14-9-3 22Z" fill="#f1cf81" stroke="#b59051" stroke-width="1.7" stroke-linejoin="round" /><path d="M105 39h35" stroke="#fff1bd" stroke-width="2" /><circle cx="122" cy="33" r="3" fill="#c7a0b6" /></g>
          <g v-if="expression.prop === 'sigh'" class="starpet-sigh" stroke="var(--starpet-accent)" stroke-width="2" stroke-linecap="round"><path d="M137 141h19q9 0 7-6m-21 12h23q10 0 10 6m-34-1h11" /></g>
          <g v-if="expression.prop === 'thought'" class="starpet-floating-prop" fill="#e9e5f4" stroke="#afa3c9" stroke-width="1.4"><path d="M164 39c-2-9 10-16 17-10 7-7 18-3 19 6 13 2 12 17 1 19-3 10-16 11-21 4-11 5-24-6-16-15Z" /><circle cx="164" cy="66" r="4" /><circle cx="156" cy="74" r="2" /><path d="M176 44h.1m7 0h.1m7 0h.1" stroke="#9688b1" stroke-width="3.4" stroke-linecap="round" /></g>
          <g v-if="expression.prop === 'magnifier'" class="starpet-tool"><path d="m164 153 17 25" stroke="#647e8d" stroke-width="9" stroke-linecap="round" /><path d="m164 153 17 25" stroke="#a7c2c9" stroke-width="4" stroke-linecap="round" /><circle cx="153" cy="137" r="22" fill="#e1f1ec" fill-opacity=".7" stroke="#617f90" stroke-width="4" /><path d="M140 133q2-9 11-10" stroke="#fffefa" stroke-width="3" stroke-linecap="round" /></g>
          <g v-if="expression.prop === 'book'" class="starpet-tool"><path d="M82 157q19-10 38 0 19-10 38 0v33q-19-8-38 1-19-9-38-1Z" fill="#8faeb9" stroke="#627c91" stroke-width="1.8" stroke-linejoin="round" /><path d="M120 157v34m-31-26q12-4 23 0m-23 7q12-4 23 0m16-7q11-4 23 0m-23 7q11-4 23 0" stroke="#edf2e4" stroke-width="1.8" stroke-linecap="round" /><path d="M143 156v13l-4-3-4 3v-15" fill="#e3b5a0" /></g>
          <g v-if="expression.prop === 'pencil'" class="starpet-tool"><rect x="91" y="159" width="52" height="32" rx="5" fill="#fff7e6" stroke="#cab794" stroke-width="1.5" transform="rotate(3 117 175)" /><path d="M100 171h26m-26 7h19" stroke="#a5b6bc" stroke-width="1.6" stroke-linecap="round" /><g class="starpet-writing-pencil"><path d="m149 144-30 33-3 10 10-4 30-33Z" fill="#e6bc77" stroke="#a88b67" stroke-width="1.6" stroke-linejoin="round" /><path d="m119 177 7 6-10 4Z" fill="#f9edce" /><path d="m116 187 2-6 4 4Z" fill="#677184" /><path d="m147 146 7 6" stroke="#ead3c5" stroke-width="4" /></g></g>
          <g v-if="expression.prop === 'palette'" class="starpet-tool"><path d="M111 158c-30 2-37 30-13 37 18 6 43-4 41-17-1-9-17-3-16-10 1-5-6-11-12-10Z" fill="#e2b794" stroke="#a57f6a" stroke-width="1.6" /><ellipse cx="127" cy="181" rx="4" ry="5" fill="#f6d999" /><circle cx="104" cy="168" r="4" fill="#bc91ad" /><circle cx="94" cy="177" r="4" fill="#87b5be" /><circle cx="103" cy="187" r="4" fill="#94b391" /><circle cx="115" cy="186" r="3.5" fill="#f6dc94" /><g class="starpet-writing-pencil"><path d="m157 143-20 33" stroke="#7c91ab" stroke-width="6" stroke-linecap="round" /><path d="m139 171-4 14q-10 4-4-7Z" fill="#a792be" stroke="#796f94" stroke-width="1.2" /><path d="m141 168-4 6" stroke="#e0d7cc" stroke-width="7" /></g></g>
          <g v-if="expression.prop === 'hourglass'" class="starpet-tool" stroke="#8c7fa6" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><path d="M164 151h28m-28 38h28M167 153c0 13 9 12 9 17s-9 4-9 17h22c0-13-9-12-9-17s9-4 9-17Z" fill="#e9e4ef" /><path d="M171 158h14l-7 8Zm7 16-8 10h16Z" fill="#dbba82" stroke="none" /></g>
          <g v-if="expression.prop === 'check'" class="starpet-result-badge"><circle cx="178" cy="170" r="19" fill="#97bfa7" stroke="#658c7e" stroke-width="1.8" /><path d="m168 170 7 7 13-15" stroke="#fff7e3" stroke-width="3.8" stroke-linecap="round" stroke-linejoin="round" /></g>
          <g v-if="expression.prop === 'alert'" class="starpet-result-badge"><path d="m178 149 20 34q3 6-4 6h-33q-7 0-4-6l18-34q2-4 3 0Z" fill="#ecc39a" stroke="#b88673" stroke-width="1.7" stroke-linejoin="round" /><path d="M177 162v12m0 8v.1" stroke="#956a68" stroke-width="3.8" stroke-linecap="round" /></g>
          <g v-if="expression.prop === 'mug'" class="starpet-tool"><path d="M137 165h10q12 12-7 18" stroke="#9eac9a" stroke-width="5" /><path d="M102 160h39v22q-1 14-20 14-18 0-19-14Z" fill="#e9edcf" stroke="#91a18b" stroke-width="1.8" /><ellipse cx="121.5" cy="160" rx="19.5" ry="4" fill="#c0a483" stroke="#91a18b" stroke-width="1.5" /><path d="m121 169 2 4 5 .8-3.5 3.5.8 5-4.3-2.3-4.3 2.3.8-5-3.5-3.5 5-.8Z" fill="#d1b575" /><path class="starpet-steam" d="M114 149q-5-5 0-10m14 10q-5-5 0-10" stroke="#a2b9af" stroke-width="1.7" stroke-linecap="round" /></g>
          <g v-for="([x, y], index) in handPositions" :key="index" class="starpet-hand" :class="{ 'starpet-wave-hand': expression.prop === 'wave' && index === 1 }">
            <g :transform="`translate(${x} ${y + frame.hand * (index ? .12 : -.12)})`">
              <circle class="starpet-hand-ball" r="9" :fill="`url(#${gradientId})`" :stroke="palette.base" stroke-width="1" />
              <ellipse cx="-3" cy="-4" rx="2.7" ry="1.7" transform="rotate(-25 -3 -4)" fill="#fffef1" opacity=".85" />
            </g>
          </g>
          </g>
        </g>
      </g>
      <g v-if="frame.burst > 0" class="starpet-touch-burst" :opacity="frame.burst" stroke-linejoin="round">
        <g v-for="(color, index) in ['#f0c778', '#b7a5d9', '#9acbbb', '#e9b2bd', '#9dc6dc', '#efdaa1']" :key="index" :transform="`translate(120 120) rotate(${index * 60 + 12}) translate(0 ${-frame.burstRadius}) rotate(${-index * 60 - 12}) scale(${.6 + frame.burst * .5})`"><path d="M0-6 2-2 6 0 2 2 0 6-2 2-6 0-2-2Z" :fill="color" stroke="#fff4db" stroke-width=".7" /></g>
      </g>
    </svg>
  </div>
</template>

<style scoped>
.starpet{display:inline-block;max-width:100%;aspect-ratio:1;vertical-align:middle;user-select:none;line-height:0;isolation:isolate}.starpet-canvas{display:block;width:100%;height:auto;overflow:visible}.starpet-body{filter:drop-shadow(0 3px 2px #443b4815)}.starpet-wave-hand{transform-origin:167px 137px}.starpet-orbiter{transform-origin:120px 122px}.starpet-writing-pencil{transform-origin:140px 167px}.starpet-confetti,.starpet-sparkles,.starpet-hearts,.starpet-floating-prop,.starpet-sleep-marks{transform-origin:120px 120px}.starpet-touch-burst{pointer-events:none}
.starpet-animated .starpet-wave-hand{animation:starpet-wave 1.9s cubic-bezier(.45,0,.2,1) infinite}.starpet-animated .starpet-orbiter{animation:starpet-orbit 3.9s linear infinite}.starpet-animated .starpet-writing-pencil{animation:starpet-pencil 1.6s cubic-bezier(.45,0,.2,1) infinite}.starpet-animated .starpet-floating-prop,.starpet-animated .starpet-sparkles,.starpet-animated .starpet-hearts{animation:starpet-float 3.2s cubic-bezier(.45,0,.2,1) infinite}.starpet-animated .starpet-sleep-marks{animation:starpet-dream 4s cubic-bezier(.45,0,.2,1) infinite}.starpet-animated .starpet-confetti{animation:starpet-confetti 2.8s cubic-bezier(.45,0,.2,1) infinite}.starpet-animated .starpet-steam,.starpet-animated .starpet-sigh{animation:starpet-dream 3.6s cubic-bezier(.45,0,.2,1) infinite}.starpet-animated .starpet-listen-lines{animation:starpet-listen 2.6s cubic-bezier(.45,0,.2,1) infinite}.starpet-animated .starpet-surprise-lines{animation:starpet-listen 3.3s cubic-bezier(.45,0,.2,1) infinite}.starpet-animated .starpet-tears{animation:starpet-tears 2.2s cubic-bezier(.45,0,.2,1) infinite}
@keyframes starpet-float{0%,100%{transform:translateY(0) rotate(-2deg)}50%{transform:translateY(-5px) rotate(3deg)}}@keyframes starpet-wave{0%,100%{transform:rotate(-8deg)}20%,55%{transform:rotate(23deg)}37%,72%{transform:rotate(-12deg)}}@keyframes starpet-orbit{to{transform:rotate(360deg)}}@keyframes starpet-pencil{0%,100%{transform:rotate(-5deg)}25%,70%{transform:rotate(7deg)}}@keyframes starpet-dream{0%,100%{transform:translateY(1px);opacity:.6}50%{transform:translateY(-5px);opacity:1}}@keyframes starpet-confetti{0%,100%{transform:translateY(-3px) rotate(-2deg);opacity:.65}50%{transform:translateY(5px) rotate(3deg);opacity:1}}@keyframes starpet-listen{0%,100%{opacity:.35}40%,65%{opacity:1}}@keyframes starpet-tears{0%,100%{transform:translateY(0);opacity:.75}55%{transform:translateY(4px);opacity:1}}
@media(prefers-reduced-motion:reduce){.starpet *{animation:none!important;transition:none!important}}
</style>
