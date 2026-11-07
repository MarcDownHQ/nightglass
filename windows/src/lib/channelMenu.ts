import { invoke } from '@tauri-apps/api/core';
import { prompt } from './dialogs.svelte';
import { library } from './library.svelte';
import { categoryKey, categoryLabel } from './lineup';
import { contextMenu, type MenuEntry } from './menuState.svelte';
import type { LiveItem } from './models';
import { nav } from './nav.svelte';
import { player } from './player.svelte';
import { store } from './store.svelte';

/** Asks a name and makes a list; with `item`, puts it in. */
export async function newList(item?: LiveItem) {
  const name = await prompt.ask('New favorites list');
  if (!name) return;
  const list = library.createList(name);
  if (item) library.toggleFavorite(list.id, item.id);
}

/** Right-click on a channel, wherever it is listed. `listId`: the favourites tab it is shown in. */
export function openChannelMenu(e: MouseEvent, item: LiveItem, listId?: string) {
  const group = item.channel.group;
  const key = group && categoryKey(group);
  const hidden = library.hiddenChannels.has(item.id);
  const groupHidden = !!key && library.hiddenGroups.has(key);
  const lists = library.favoriteLists;
  const entries: MenuEntry[] = [
    { label: 'Play', icon: '', action: () => void store.tune(item) },
    {
      label: 'Show in the guide',
      icon: '',
      action: () => {
        void player.collapse();
        nav.guideChannel = item.id;
        nav.section = 'guide';
      },
    },
    { label: 'Copy the stream URL', icon: '', action: () => void copyUrl(item) },
  ];
  if (listId) {
    entries.push(
      { heading: 'Order' },
      { label: 'Move to the top', icon: '', action: () => library.moveFavorite(listId, item.id, -Infinity) },
      { label: 'Move up', icon: '', action: () => library.moveFavorite(listId, item.id, -1) },
      { label: 'Move down', icon: '', action: () => library.moveFavorite(listId, item.id, 1) },
    );
  }
  entries.push({ heading: 'Favorites' });
  for (const list of lists) {
    const inside = list.items.includes(item.id);
    entries.push({
      label: `${inside ? 'Remove from' : 'Add to'} ${list.name}`,
      icon: inside ? '' : '',
      action: () => library.toggleFavorite(list.id, item.id),
    });
  }
  entries.push({ label: 'New list…', icon: '', action: () => void newList(item) });
  entries.push(
    { heading: 'Hide' },
    hidden
      ? { label: 'Show this channel again', icon: '', action: () => library.toggleHidden(library.hiddenChannels, item.id) }
      : { label: 'Hide this channel', icon: '', action: () => library.toggleHidden(library.hiddenChannels, item.id) },
  );
  if (key) {
    const name = library.categoryNames[key] ?? categoryLabel(group);
    entries.push(
      groupHidden
        ? { label: `Show “${name}” again`, icon: '', action: () => library.toggleHidden(library.hiddenGroups, key) }
        : { label: `Hide “${name}”`, icon: '', action: () => library.toggleHidden(library.hiddenGroups, key) },
    );
  }
  contextMenu.open(e, entries);
}

/** The feeds of a merged channel: pick one. */
export function openVariantMenu(e: MouseEvent, row: LiveItem) {
  const playing = player.current?.item?.id;
  contextMenu.open(e, [
    { heading: 'Feeds' },
    ...(row.variants ?? [row]).map((v) => ({
      label: `${v.channel.name} · ${v.source.name}`,
      checked: v.id === playing,
      action: () => void store.tune({ ...v, variants: row.variants }),
    })),
  ]);
}

/** Stalker links are resolved first: the stored one is a portal command, not a URL a player can open. */
async function copyUrl(item: LiveItem) {
  try {
    const url = 'stalker' in item.source.kind ? await invoke<string>('stream_url', { source: item.source, channel: item.channel }) : item.channel.stream;
    await navigator.clipboard.writeText(url);
    player.notify('Stream URL copied');
  } catch (e) {
    player.notify(`Copy failed: ${e}`);
  }
}
