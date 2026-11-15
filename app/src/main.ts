import './assets/fonts/geist.css';
import { mount } from 'svelte';
import App from './App.svelte';
import { installErrorForwarding } from './lib/errorForwarding.ts';
import { suppressNativeContextMenu } from './lib/native-menu.ts';
import { invoke } from './services/command-error.ts';
import { getIsTauri, startLoadStateEarly } from './services/tauri.svelte.ts';

if (getIsTauri()) installErrorForwarding(window, (level, message) => invoke('log_frontend', { level, message }));

startLoadStateEarly();
loadEveryFontSubsetNow();

function loadEveryFontSubsetNow() {
  for (const face of document.fonts) if (face.status === 'unloaded') void face.load();
}

if (import.meta.env.PROD) suppressNativeContextMenu();

const app = mount(App, { target: document.getElementById('app')! });

export default app;
