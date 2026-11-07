<script lang="ts">
  import { t } from './i18n.svelte';
  import { isPermissionGranted, requestPermission, sendNotification } from '@tauri-apps/plugin-notification';
  import { onMount } from 'svelte';
  import { fly } from 'svelte/transition';
  import { library, type Reminder } from './library.svelte';
  import { nav } from './nav.svelte';
  import { player } from './player.svelte';
  import { store } from './store.svelte';
  import { hm } from './theme';

  /** Programmes the user asked about, starting now: a Windows notification and a banner with Watch. */
  let due = $state<Reminder[]>([]);

  async function notify(r: Reminder) {
    try {
      let granted = await isPermissionGranted();
      if (!granted) granted = (await requestPermission()) === 'granted';
      if (granted) sendNotification({ title: t('{title} starts', { title: r.title }), body: t('{time} on {channel}', { time: hm(r.start), channel: r.channelName }) });
    } catch (e) {
      console.warn(`notification: ${e}`);
    }
  }

  function check() {
    for (const r of library.takeDue(Date.now() / 1000)) {
      due = [...due, r];
      void notify(r);
    }
  }

  onMount(() => {
    check();
    const timer = setInterval(check, 15_000);
    return () => clearInterval(timer);
  });

  function watch(r: Reminder) {
    due = due.filter((x) => x !== r);
    void player.collapse();
    nav.focusChannel = r.channel;
    nav.section = 'live';
    if (!store.channels.some((c) => c.id === r.channel)) player.notify(t('{name} is not in the sources shown', { name: r.channelName }));
  }
</script>

{#if due.length}
  <div class="banners">
    {#each due as r (r.channel + r.start)}
      <div class="banner" transition:fly={{ y: -20, duration: 200 }}>
        <span class="icon bell">&#xEA8F;</span>
        <div class="text">
          <b>{r.title}</b>
          <small>{hm(r.start)} · {r.channelName}</small>
        </div>
        <button class="watch" onclick={() => watch(r)}>{t('Watch')}</button>
        <button class="icon close" title={t('Dismiss')} onclick={() => (due = due.filter((x) => x !== r))}>&#xE8BB;</button>
      </div>
    {/each}
  </div>
{/if}

<style>
  .banners {
    position: fixed;
    top: 73px;
    right: 24px;
    z-index: 95;
    display: flex;
    flex-direction: column;
    gap: 10px;
  }
  .banner {
    width: 380px;
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 12px 14px;
    border-radius: 12px;
    background: var(--ink);
    border: 1px solid rgb(249 240 2 / 0.5);
    box-shadow: 0 0 18px rgb(249 240 2 / 0.15);
  }
  .bell {
    font-size: 18px;
    color: var(--acid);
  }
  .text {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .text b,
  .text small {
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .text small {
    color: var(--dim);
  }
  .watch {
    padding: 6px 14px;
    border-radius: 999px;
    background: var(--acid);
    color: var(--ink);
    font-weight: 700;
  }
  .close {
    padding: 6px;
    font-size: 10px;
    color: var(--dim);
  }
</style>
