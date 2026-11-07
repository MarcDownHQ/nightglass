import { invoke } from '@tauri-apps/api/core';
import { contextMenu } from './menuState.svelte';
import { library } from './library.svelte';
import type { LiveItem } from './models';
import { nav } from './nav.svelte';
import { player } from './player.svelte';
import { store } from './store.svelte';

/** Right-click on a channel, wherever it is listed. */
export function openChannelMenu(e: MouseEvent, item: LiveItem) {
  const group = item.channel.group;
  const hidden = library.hiddenChannels.has(item.id);
  const groupHidden = !!group && library.hiddenGroups.has(group);
  contextMenu.open(e, [
    { label: 'Play', icon: '', action: () => void store.tune(item) },
    store.favorites.has(item.id)
      ? { label: 'Remove from favorites', icon: '', action: () => store.toggleFavorite(item) }
      : { label: 'Add to favorites', icon: '', action: () => store.toggleFavorite(item) },
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
    hidden
      ? { label: 'Show this channel again', icon: '', action: () => library.toggleHidden(library.hiddenChannels, item.id) }
      : { label: 'Hide this channel', icon: '', action: () => library.toggleHidden(library.hiddenChannels, item.id) },
    ...(group
      ? [
          groupHidden
            ? { label: `Show “${group}” again`, icon: '', action: () => library.toggleHidden(library.hiddenGroups, group) }
            : { label: `Hide “${group}”`, icon: '', action: () => library.toggleHidden(library.hiddenGroups, group) },
        ]
      : []),
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
