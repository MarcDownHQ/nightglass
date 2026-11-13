<script lang="ts">
  import { i18n, MAC, t, type Language } from './i18n.svelte';
  import { getVersion } from '@tauri-apps/api/app';
  import { organizer, panels } from './dialogs.svelte';
  import Modal from './Modal.svelte';
  import { options, type StartScreen } from './options.svelte';
  import { exportSettings, importSettings } from './settings';
  import { updater } from './updater.svelte';

  let version = $state('');
  $effect(() => void getVersion().then((v) => (version = v)));

  const STARTS: [StartScreen, string, string][] = [
    ['home', 'Home', 'The last channel, recents, favourites and what was left half-watched'],
    ['live', 'Live', 'The channel list'],
    ['last', 'Last channel', 'Live, playing the channel watched last'],
  ];
</script>

<Modal title={t('Settings')} close={() => (panels.settings = false)}>
  <div class="settings">
    <section>
      <h3 class="mono">{t('At launch')}</h3>
      {#each STARTS as [value, label, hint] (value)}
        <label class="choice">
          <input type="radio" name="start" checked={options.startScreen === value} onchange={() => options.set('startScreen', value)} />
          <span><b>{t(label)}</b><small>{t(hint)}</small></span>
        </label>
      {/each}
      <label class="choice">
        <input type="checkbox" checked={options.startFullscreen} onchange={(e) => options.set('startFullscreen', e.currentTarget.checked)} />
        <span><b>{t('Fullscreen window')}</b><small>{t('F11 switches at any time')}</small></span>
      </label>
    </section>

    <section>
      <h3 class="mono">{t('Sources')}</h3>
      <label class="select">
        {t('Reload catalogs and guides')}
        <select value={options.refreshHours} onchange={(e) => options.set('refreshHours', Number(e.currentTarget.value))}>
          <option value={6}>{t('every 6 hours')}</option>
          <option value={12}>{t('every 12 hours')}</option>
          <option value={24}>{t('every day')}</option>
          <option value={0}>{t('only at launch')}</option>
        </select>
      </label>
    </section>

    <section>
      <h3 class="mono">{t('Language')}</h3>
      <label class="select">
        <select value={i18n.choice} onchange={(e) => i18n.set(e.currentTarget.value as Language)}>
          <option value="auto">{t(MAC ? 'Same as macOS' : 'Same as Windows')}</option>
          <option value="en">English</option>
          <option value="fr">Français</option>
        </select>
      </label>
    </section>

    <section>
      <h3 class="mono">{t('Interface size')}</h3>
      <div class="zoom">
        <button onclick={() => options.stepZoom(-0.1)} title={t('Smaller (Ctrl −)')} class="icon">&#xE738;</button>
        <span class="mono">{Math.round(options.zoom * 100)} %</span>
        <button onclick={() => options.stepZoom(0.1)} title={t('Larger (Ctrl +)')} class="icon">&#xE710;</button>
        <button class="link" disabled={options.zoom === 1} onclick={() => options.set('zoom', 1)}>{t('Reset (Ctrl 0)')}</button>
      </div>
      <small class="hint">{t('Larger for a TV across the room.')}</small>
    </section>

    <section>
      <h3 class="mono">{t('Channels and parental control')}</h3>
      <button
        class="link"
        onclick={() => {
          panels.settings = false;
          organizer.open = true;
        }}>{t('Categories, favourites lists and PIN…')}</button
      >
    </section>

    <section>
      <h3 class="mono">{t('Settings file')}</h3>
      <div class="row">
        <button class="link" onclick={exportSettings}>{t('Export…')}</button>
        <button class="link" onclick={importSettings}>{t('Import…')}</button>
      </div>
      <small class="hint">{t('Sources, favourites, lists, hidden categories, progress: to move to another computer.')}</small>
    </section>

    <section>
      <h3 class="mono">Nightglass {version}</h3>
      <div class="row">
        <button
          class="link"
          onclick={() => {
            panels.settings = false;
            panels.help = true;
          }}>{t('Keyboard shortcuts (?)')}</button
        >
        {#if updater.available}
          <button class="link" onclick={() => updater.install()}>{t('Install {version}', { version: updater.available.version })}</button>
        {/if}
      </div>
    </section>
  </div>
</Modal>

<style>
  .settings {
    display: flex;
    flex-direction: column;
    gap: 22px;
  }
  section {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  h3 {
    margin: 0;
    font-size: 12px;
    font-weight: 600;
    color: var(--acid);
  }
  .choice {
    display: flex;
    align-items: flex-start;
    gap: 10px;
    cursor: pointer;
  }
  /* Radios and the checkbox share one box, centred on the first line of the label. */
  .choice input {
    flex: none;
    width: 16px;
    height: 16px;
    margin: 2px 0 0;
    accent-color: var(--magenta);
  }
  .choice span {
    display: flex;
    flex-direction: column;
  }
  .choice b {
    font-size: 16px;
    line-height: 20px;
  }
  .choice small,
  .hint {
    font-size: 13px;
    color: var(--dim);
  }
  .select {
    display: flex;
    align-items: center;
    gap: 10px;
    font-size: 15px;
  }
  select {
    padding: 5px 8px;
    border-radius: 6px;
    border: 1px solid rgb(233 236 242 / 0.2);
    background: var(--night);
    color: var(--text);
    font: inherit;
  }
  .zoom {
    display: flex;
    align-items: center;
    gap: 12px;
  }
  .zoom button:not(.link) {
    width: 34px;
    height: 34px;
    border-radius: 50%;
    border: 1px solid rgb(233 236 242 / 0.2);
    font-size: 12px;
  }
  .zoom .link {
    align-self: center;
  }
  .zoom span {
    min-width: 56px;
    text-align: center;
  }
  .row {
    display: flex;
    gap: 20px;
  }
  /* 19 px: an odd line for the capitals, an odd number of pixels tall, to sit in its middle. */
  .link {
    align-self: flex-start;
    color: var(--cyan);
    font-size: 15px;
    line-height: 19px;
  }
  .link:disabled {
    opacity: 0.4;
  }
</style>
