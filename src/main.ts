import { createApp } from 'vue';
import './starpet-shell.css';

const starpet = new URLSearchParams(window.location.search).get('view') === 'starpet';
if (starpet) document.documentElement.dataset.view = 'starpet';

async function mount() {
  // The companion does not need the clipboard workspace, editors or their styles.
  if (!starpet) await import('./styles.css');
  const { default: root } = starpet
    ? await import('./components/StarPetDesktop.vue')
    : await import('./App.vue');
  createApp(root).mount('#app');
}
void mount();
