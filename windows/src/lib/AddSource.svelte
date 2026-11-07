<script lang="ts">
  import { t } from './i18n.svelte';
  import { friendly } from './errors';
  import { invoke } from '@tauri-apps/api/core';
  import HudPanel from './HudPanel.svelte';
  import type { Catalog, Source, SourceKind } from './models';
  import NeonTab from './NeonTab.svelte';
  import { store } from './store.svelte';

  const kinds = ['M3U', 'Xtream', 'Stalker'] as const;
  let kind = $state<(typeof kinds)[number]>('M3U');
  let name = $state('');
  let url = $state('');
  let epg = $state('');
  let user = $state('');
  let pass = $state('');
  let mac = $state('00:1A:79:');
  let message = $state<{ text: string; error: boolean } | null>(null);
  let busy = $state(false);

  function preset() {
    name = 'iptv-org · France';
    url = 'https://iptv-org.github.io/iptv/countries/fr.m3u';
    epg = 'https://xmltvfr.fr/xmltv/xmltv.xml';
  }

  async function connect() {
    busy = true;
    message = null;
    try {
      // Also turns a Windows path (C:\…\list.m3u) into a file URL.
      const location = await invoke<string>('parse_location', { input: url });
      const sourceKind: SourceKind =
        kind === 'M3U'
          ? { m3u: { url: location, epg: epg.trim() ? await invoke<string>('parse_location', { input: epg }) : null } }
          : kind === 'Xtream'
            ? { xtream: { server: location, username: user, password: pass } }
            : { stalker: { portal: location, mac: mac.trim() } };
      const source: Source = { id: crypto.randomUUID(), name: name.trim() || new URL(location).host || kind, kind: sourceKind };
      const catalog = await invoke<Catalog>('catalog', { source });
      store.add(source, catalog);
      message = {
        text: t('Added · {channels} channels, {movies} movies, {series} series', { channels: catalog.channels.length, movies: catalog.movies.length, series: catalog.series.length }),
        error: false,
      };
      name = url = epg = user = pass = '';
    } catch (e) {
      message = { text: friendly(String(e)), error: true };
    } finally {
      busy = false;
    }
  }
</script>

<HudPanel caption={t('Add a source')}>
  <form
    onsubmit={(e) => {
      e.preventDefault();
      void connect();
    }}
  >
    <div class="kinds">
      {#each kinds as k (k)}
        <NeonTab title={k} selected={k === kind} onclick={() => ((kind = k), (message = null))} />
      {/each}
    </div>
    <label><span>{t('Name')}</span><input bind:value={name} placeholder={t('My provider')} spellcheck="false" /></label>
    {#if kind === 'M3U'}
      <label><span>{t('Playlist URL or file')}</span><input bind:value={url} placeholder="https://… .m3u" spellcheck="false" /></label>
      <label><span>{t('Guide URL (XMLTV, optional)')}</span><input bind:value={epg} placeholder="https://… .xml" spellcheck="false" /></label>
      <button type="button" class="preset" onclick={preset}>{t('Use iptv-org France')}</button>
    {:else if kind === 'Xtream'}
      <label><span>{t('Server')}</span><input bind:value={url} placeholder="http://provider:8080" spellcheck="false" /></label>
      <label><span>{t('Username')}</span><input bind:value={user} autocomplete="off" spellcheck="false" /></label>
      <label><span>{t('Password')}</span><input type="password" bind:value={pass} autocomplete="off" /></label>
    {:else}
      <label><span>{t('Portal URL')}</span><input bind:value={url} placeholder="http://portal/c/" spellcheck="false" /></label>
      <label><span>{t('MAC address')}</span><input bind:value={mac} placeholder="00:1A:79:XX:XX:XX" spellcheck="false" /></label>
    {/if}
    <div>
      <button type="submit" class="connect" disabled={busy || !url.trim()}>{busy ? t('Connecting…') : t('Connect')}</button>
    </div>
    {#if message}<p class="message" class:error={message.error}>{message.text}</p>{/if}
  </form>
</HudPanel>

<style>
  form {
    display: flex;
    flex-direction: column;
    gap: 14px;
    padding: 18px;
  }
  .kinds {
    display: flex;
    gap: 8px;
  }
  label {
    display: flex;
    flex-direction: column;
    gap: 5px;
  }
  label span {
    font-size: 13px;
    font-weight: 500;
    color: var(--dim);
  }
  input {
    font-family: var(--mono);
    font-size: 13px;
    color: var(--text);
    padding: 9px;
    border-radius: 8px;
    background: rgb(5 1 15 / 0.6);
    border: 1px solid rgb(5 217 232 / 0.25);
    outline: none;
    user-select: text;
  }
  input:focus {
    border-color: var(--cyan);
    box-shadow: 0 0 8px rgb(5 217 232 / 0.35);
  }
  input::placeholder {
    color: rgb(243 238 255 / 0.3);
  }
  .preset {
    align-self: flex-start;
    font-size: 14px;
    line-height: 18px;
    font-weight: 500;
    color: var(--cyan);
  }
  .connect {
    padding: 10px 22px;
    border-radius: 999px;
    background: var(--magenta);
    color: var(--ink);
    font-size: 18px;
    font-weight: 700;
    box-shadow: 0 0 14px rgb(255 42 109 / 0.6);
    transition: opacity 0.15s, box-shadow 0.15s;
  }
  .connect:hover:not(:disabled) {
    box-shadow: 0 0 24px rgb(255 42 109 / 0.85);
  }
  .connect:disabled {
    opacity: 0.5;
    cursor: default;
  }
  .message {
    margin: 0;
    font-size: 14px;
    font-weight: 500;
    color: var(--cyan);
  }
  .message.error {
    color: var(--magenta);
  }
</style>
