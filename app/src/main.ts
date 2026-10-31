import './assets/fonts/geist.css';
import { mount } from 'svelte';
import App from './App.svelte';
import { suppressNativeContextMenu } from './lib/native-menu.ts';
import { startLoadStateEarly } from './services/tauri.svelte.ts';

startLoadStateEarly();

if (import.meta.env.PROD) suppressNativeContextMenu();

const app = mount(App, { target: document.getElementById('app')! });

export default app;
