import './module-sidebar.css';
import { el } from '../design-system/utils/dom.js';
import { createSidebarSection, createSidebarRow } from '../design-system/index.js';

export const meta = { title: 'Module · Main Sidebar (240w nav)', layer: 'module' };

/*
 * The main navigation sidebar: window chrome dots, then Favorites / Library /
 * Playlists sections, with USB Devices pinned to the bottom (Pencil "Main Sidebar").
 * Exports buildSidebar() for the Browse composite.
 */
function trafficLights() {
  return el('div', { class: 'ff-sidebar__chrome' }, [
    el('span', { class: 'ff-sidebar__dot ff-sidebar__dot--close' }),
    el('span', { class: 'ff-sidebar__dot ff-sidebar__dot--min' }),
    el('span', { class: 'ff-sidebar__dot ff-sidebar__dot--max' }),
  ]);
}

export function buildSidebar() {
  const favorites = createSidebarSection({
    label: 'FAVORITES',
    rows: [
      createSidebarRow({ kind: 'favorite', digit: 1, label: 'Peak Time', count: 24 }).element,
      createSidebarRow({ kind: 'favorite', digit: 2, label: 'Warmup', count: 18 }).element,
    ],
  }).element;

  const library = createSidebarSection({
    label: 'LIBRARY',
    rows: [
      createSidebarRow({ kind: 'leaf', icon: 'list', label: 'All Tracks', count: 1284, state: 'active' }).element,
      createSidebarRow({ kind: 'leaf', icon: 'clock-3', label: 'Recently Added', count: 52 }).element,
      createSidebarRow({ kind: 'leaf', icon: 'disc', label: 'Unfiled', count: 18 }).element,
    ],
  }).element;

  const playlists = createSidebarSection({
    label: 'PLAYLISTS',
    showAdd: true,
    rows: [
      createSidebarRow({ kind: 'playlist', label: 'Saturday', count: 64 }).element,
      createSidebarRow({ kind: 'playlist', label: 'Sunday Closing', count: 40 }).element,
      createSidebarRow({ kind: 'playlist', label: 'Festival Prep', count: 28 }).element,
    ],
  }).element;

  const usb = createSidebarSection({
    label: 'USB DEVICES',
    rows: [
      createSidebarRow({ kind: 'usb', label: 'SanDisk 64GB', status: 'online' }).element,
      createSidebarRow({ kind: 'usb', label: 'VAULT666', status: 'offline' }).element,
    ],
  }).element;

  const top = el('div', { class: 'ff-sidebar__top' }, [favorites, library, playlists]);
  const bottom = el('div', { class: 'ff-sidebar__bottom' }, [usb]);

  return el('div', { class: 'ff-sidebar' }, [trafficLights(), top, bottom]);
}

export default function render() {
  return buildSidebar();
}
