<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { t } from './i18n.svelte';
  import HudPanel from './HudPanel.svelte';
  import type { PluginStatus } from './models';

  let dir = $state('');
  let plugins = $state<PluginStatus[]>([]);

  void invoke<{ dir: string; plugins: PluginStatus[] }>('plugins').then((r) => ({ dir, plugins } = r));
</script>

<HudPanel caption={t('Plugins')} trailing={String(plugins.length)}>
  <div class="plugins">
    {#each plugins as p (p.file)}
      <p class:error={p.error}><b>{p.provider ?? p.file}</b> <small title={p.error ?? p.file}>{p.error ?? p.file}</small></p>
    {/each}
    <small>{t('Source types from .wasm files in this folder, loaded at launch:')}</small>
    <code>{dir}</code>
  </div>
</HudPanel>

<style>
  .plugins {
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 14px 18px;
  }
  p {
    margin: 0;
    display: flex;
    gap: 10px;
    align-items: baseline;
    min-width: 0;
  }
  p small {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  small {
    font-size: 13px;
    color: var(--dim);
  }
  .error small {
    color: var(--live);
  }
  code {
    font-size: 12px;
    color: var(--cyan);
    user-select: text;
    overflow-wrap: anywhere;
  }
</style>
