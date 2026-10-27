import { mount } from 'svelte';
import App from './App.svelte';
import { suppressNativeContextMenu } from './lib/native-menu.ts';

if (import.meta.env.PROD) suppressNativeContextMenu();

const app = mount(App, { target: document.getElementById('app')! });

export default app;
