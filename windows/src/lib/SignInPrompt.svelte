<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { t } from './i18n.svelte';
  import type { Prompt } from './models';
  import { player } from './player.svelte';
  import { retryLater } from './signIn';

  /**
   * A plugin waiting for the user: what to do, where, the code to enter. `retry` asks again, every `retryIn` seconds
   * while shown, unless the last ask is still `busy`.
   */
  let { prompt, busy, retry }: { prompt: Prompt; busy: boolean; retry: () => void } = $props();

  $effect(() => retryLater(prompt.retryIn, () => !busy && retry()));

  async function copy(code: string) {
    try {
      await navigator.clipboard.writeText(code);
      player.notify(t('Code copied'));
    } catch (e) {
      player.notify(t('Copy failed: {error}', { error: String(e) }));
    }
  }

  function open(e: MouseEvent, url: string) {
    e.preventDefault();
    e.stopPropagation();
    invoke('open_link', { url }).catch((err) => player.notify(String(err)));
  }
</script>

<div class="prompt">
  <p>{prompt.message}</p>
  {#if prompt.url}
    {@const url = prompt.url}
    <a href={url} title={t('Open in the browser')} onclick={(e) => open(e, url)}>{url}</a>
  {/if}
  <div class="actions">
    {#if prompt.code}
      {@const code = prompt.code}
      <button
        class="code mono"
        title={t('Copy the code')}
        onclick={(e) => {
          e.stopPropagation();
          void copy(code);
        }}>{code}</button
      >
    {/if}
    <button
      class="retry"
      disabled={busy}
      onclick={(e) => {
        e.stopPropagation();
        retry();
      }}>{t('Retry')}</button
    >
  </div>
</div>

<style>
  .prompt {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 8px;
    margin-top: 4px;
  }
  p {
    margin: 0;
    font-size: 15px;
    color: var(--acid);
  }
  a {
    font-size: 15px;
    color: var(--cyan);
    word-break: break-all;
  }
  .actions {
    display: flex;
    align-items: center;
    gap: 12px;
  }
  .code {
    padding: 6px 16px;
    border-radius: 8px;
    border: 1px solid rgb(233 236 242 / 0.16);
    font-size: 28px;
    font-weight: 700;
    letter-spacing: 0.12em;
    color: var(--text);
    user-select: all;
  }
  .code:hover {
    border-color: var(--cyan);
  }
  .retry {
    padding: 6px 16px;
    border-radius: 999px;
    border: 1px solid rgb(233 236 242 / 0.1);
    font-size: 14px;
    color: var(--cyan);
  }
  .retry:hover:not(:disabled) {
    border-color: var(--cyan);
  }
  .retry:disabled {
    opacity: 0.5;
  }
</style>
