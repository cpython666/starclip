import { createApp } from 'vue';
import App from './App.vue';
import StarPetDesktop from './components/StarPetDesktop.vue';
import './styles.css';
const starpet = new URLSearchParams(window.location.search).get('view') === 'starpet';
if (starpet) document.documentElement.dataset.view = 'starpet';
createApp(starpet ? StarPetDesktop : App).mount('#app');
