<script lang="ts">
  import { t } from './i18n.svelte';
  import Modal from './Modal.svelte';
  import { widevine } from './widevine.svelte';

  const mb = (bytes: number) => (bytes / 1e6).toFixed(1);

  const status = $derived.by(() => {
    const p = widevine.progress;
    if (!p || p.stage === 'checking') return t('Asking Google for the module…');
    if (p.stage === 'verifying') return t('Checking Google’s signature…');
    return t('Downloading: {received} of {total} MB', { received: mb(p.received), total: mb(p.total) });
  });
  const share = $derived(widevine.progress?.stage === 'downloading' && widevine.progress.total ? widevine.progress.received / widevine.progress.total : 0);
</script>

{#if widevine.step}
  <Modal title={t('Protected content (Widevine)')} close={() => widevine.close()} width={520}>
    <div class="widevine">
      {#if widevine.step === 'blocked'}
        <p class="error">{t('{name} is protected, and protected content is off.', { name: widevine.title })}</p>
      {/if}
      {#if widevine.step === 'blocked' || widevine.step === 'explain'}
        <p>{t('Some channels, movies and episodes are encrypted with Widevine, Google’s DRM. To play them, Nightglass downloads Google’s Widevine module (about 20 MB) from Google and keeps it on this computer.')}</p>
        <p>{t('It is a closed program from Google, under Google’s own licence: Nightglass doesn’t ship it, and no plugin can download it.')}</p>
        <p>{t('A computer gets Widevine’s software level (L3): services often hold it to SD or 720p, and some refuse it.')}</p>
        <p class="dim">{t('Turning the setting off deletes the module.')}</p>
        <div class="buttons">
          <button onclick={() => widevine.close()}>{t('Cancel')}</button>
          <button class="ok" onclick={() => void widevine.install()}>{widevine.step === 'blocked' ? t('Turn on and play') : t('Download')}</button>
        </div>
      {:else if widevine.step === 'installing'}
        <p class="mono">{status}</p>
        <div class="bar" class:busy={widevine.progress?.stage !== 'downloading'}><span style:width="{share * 100}%"></span></div>
        <div class="buttons">
          <button onclick={() => widevine.close()}>{t('Cancel')}</button>
        </div>
      {:else if widevine.step === 'failed'}
        <p class="error">{t('The Widevine module couldn’t be installed: {error}', { error: widevine.error ?? '' })}</p>
        <div class="buttons">
          <button onclick={() => widevine.close()}>{t('Cancel')}</button>
          <button class="ok" onclick={() => void widevine.install()}>{t('Retry')}</button>
        </div>
      {/if}
    </div>
  </Modal>
{/if}

<style>
  .widevine {
    display: flex;
    flex-direction: column;
    gap: 12px;
  }
  p {
    margin: 0;
    font-size: 15px;
    line-height: 1.45;
  }
  .dim {
    font-size: 13px;
    color: var(--dim);
  }
  .error {
    color: #ff9a8d;
    font-weight: 600;
  }
  .bar {
    height: 6px;
    border-radius: 3px;
    background: rgb(233 236 242 / 0.12);
    overflow: hidden;
  }
  .bar span {
    display: block;
    height: 100%;
    background: var(--cyan);
  }
  .bar.busy span {
    width: 30% !important;
    animation: slide 1.2s ease-in-out infinite alternate;
  }
  @keyframes slide {
    from {
      translate: 0;
    }
    to {
      translate: 233%;
    }
  }
  .buttons {
    display: flex;
    justify-content: flex-end;
    gap: 10px;
    margin-top: 4px;
  }
  .buttons button {
    padding: 7px 16px;
    border-radius: 999px;
    font-size: 15px;
    font-weight: 600;
    color: var(--dim);
  }
  .buttons .ok {
    background: var(--cyan);
    color: var(--ink);
  }
</style>
