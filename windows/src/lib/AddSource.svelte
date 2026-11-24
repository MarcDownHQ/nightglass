<script lang="ts">
  import { t } from './i18n.svelte';
  import { friendly, promptOf } from './errors';
  import { invoke } from '@tauri-apps/api/core';
  import HudPanel from './HudPanel.svelte';
  import type { Catalog, Field, Preset, ProviderInfo, Source } from './models';
  import NeonTab from './NeonTab.svelte';
  import { store } from './store.svelte';

  let chosen = $state<string | null>(null);
  const provider = $derived(store.providers.find((p) => p.id === chosen) ?? store.providers[0]);
  let name = $state('');
  /** What was typed, per provider and field. */
  let typed = $state<Record<string, Record<string, string>>>({});
  let message = $state<{ text: string; error: boolean } | null>(null);
  let busy = $state(false);

  const value = (p: ProviderInfo, f: Field) => typed[p.id]?.[f.key] ?? f.default ?? '';
  const missing = $derived(!provider || provider.fields.some((f) => f.required && !value(provider, f).trim()));

  function set(f: Field, v: string) {
    typed[provider.id] = { ...typed[provider.id], [f.key]: v };
  }

  function choose(p: ProviderInfo) {
    chosen = p.id;
    message = null;
  }

  function preset(p: Preset) {
    name = p.name;
    typed[provider.id] = { ...p.config };
  }

  /** Addresses go through `parse_location`, which also turns a Windows path (C:\…\list.m3u) into a file URL. */
  async function settings(p: ProviderInfo): Promise<Record<string, string>> {
    const config: Record<string, string> = {};
    for (const f of p.fields) {
      const raw = value(p, f);
      if (!raw.trim()) continue;
      config[f.key] =
        f.kind === 'url' || f.kind === 'location'
          ? await invoke<string>('parse_location', { input: raw, files: f.kind === 'location' })
          : f.kind === 'mac'
            ? raw.trim()
            : raw;
    }
    return config;
  }

  async function connect() {
    const p = provider;
    busy = true;
    message = null;
    let source: Source | undefined;
    try {
      const config = await settings(p);
      const address = p.fields.find((f) => (f.kind === 'url' || f.kind === 'location') && config[f.key]);
      const host = address ? new URL(config[address.key]).host : '';
      source = { id: crypto.randomUUID(), name: name.trim() || host || p.name, provider: p.id, config };
      const catalog = await invoke<Catalog>('catalog', { source });
      store.add(source, catalog);
      message = {
        text: t('Added · {channels} channels, {movies} movies, {series} series', { channels: catalog.channels.length, movies: catalog.movies.length, series: catalog.series.length }),
        error: false,
      };
      name = '';
      typed[p.id] = {};
    } catch (e) {
      const prompt = promptOf(e);
      if (prompt && source) {
        store.addWaiting(source, prompt);
        message = { text: t('Added · waiting for you: see the list of sources'), error: false };
        name = '';
        typed[p.id] = {};
      } else message = { text: friendly(String(e)), error: true };
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
      {#each store.providers as p (p.id)}
        <NeonTab title={p.name} selected={p.id === provider?.id} onclick={() => choose(p)} />
      {/each}
    </div>
    <label><span>{t('Name')}</span><input bind:value={name} placeholder={t('My provider')} spellcheck="false" /></label>
    {#if provider}
      {#each provider.fields as f (provider.id + f.key)}
        <label>
          <span>{t(f.label)}</span>
          <input
            type={f.kind === 'secret' ? 'password' : 'text'}
            value={value(provider, f)}
            oninput={(e) => set(f, e.currentTarget.value)}
            placeholder={f.placeholder ?? ''}
            autocomplete="off"
            spellcheck="false"
          />
        </label>
      {/each}
      {#each provider.presets as p (p.label)}
        <button type="button" class="preset" onclick={() => preset(p)}>{t(p.label)}</button>
      {/each}
    {/if}
    <div>
      <button type="submit" class="connect" disabled={busy || missing}>{busy ? t('Connecting…') : t('Connect')}</button>
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
    font-weight: 600;
    color: var(--dim);
  }
  input {

    font-size: 13px;
    color: var(--text);
    padding: 9px;
    border-radius: 8px;
    background: rgb(10 12 17 / 0.6);
    border: 1px solid rgb(233 236 242 / 0.1);
    outline: none;
    user-select: text;
  }
  input:focus {
    border-color: var(--cyan);
  }
  input::placeholder {
    color: rgb(233 236 242 / 0.3);
  }
  .preset {
    align-self: flex-start;
    font-size: 14px;
    line-height: 18px;
    font-weight: 600;
    color: var(--cyan);
  }
  .connect {
    padding: 10px 22px;
    border-radius: 999px;
    background: var(--magenta);
    color: var(--ink);
    font-size: 18px;
    font-weight: 700;
    transition: opacity 0.15s, box-shadow 0.15s;
  }
  .connect:disabled {
    opacity: 0.5;
    cursor: default;
  }
  .message {
    margin: 0;
    font-size: 14px;
    font-weight: 600;
    color: var(--cyan);
  }
  .message.error {
    color: var(--live);
  }
</style>
