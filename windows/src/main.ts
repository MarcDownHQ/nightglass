import { mount } from 'svelte';
import { getCurrentWindow } from '@tauri-apps/api/window';
import './app.css';

// The PiP window loads this page too: it gets its own small view, and none of the app's modules
// (loading the player would stop mpv).
const target = document.getElementById('app')!;
if (getCurrentWindow().label === 'pip') void import('./PipView.svelte').then((m) => mount(m.default, { target }));
else void import('./App.svelte').then((m) => mount(m.default, { target }));
