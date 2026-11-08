<script lang="ts">
  import { t } from './i18n.svelte';
  import AddSource from './AddSource.svelte';
  import HudPanel from './HudPanel.svelte';
  import { kindLabel, type Source } from './models';
  import { store } from './store.svelte';
  import Tag from './Tag.svelte';
  import { friendly } from './errors';
  import type { Account } from './models';
  import { exportSettings, importSettings } from './settings';
  import { neon } from './theme';

  /** "Expires 12 Dec 2026 · 1/2 connections": what the provider says about the subscription. */
  function accountLine(a: Account): string {
    const parts: string[] = [];
    if (a.status && a.status !== 'Active') parts.push(a.status);
    if (a.trial) parts.push(t('Trial'));
    if (a.expires) {
      const days = Math.ceil((a.expires * 1000 - Date.now()) / 86_400_000);
      const date = new Date(a.expires * 1000).toLocaleDateString([], { day: 'numeric', month: 'short', year: 'numeric' });
      parts.push(
        days < 0
          ? t('Expired on {date}', { date })
          : days <= 14
            ? t(days === 1 ? 'Expires in {n} day ({date})' : 'Expires in {n} days ({date})', { n: days, date })
            : t('Expires {date}', { date }),
      );
    } else parts.push(t('No expiry'));
    if (a.maxConnections) parts.push(t('{active}/{max} connections', { active: a.activeConnections ?? 0, max: a.maxConnections }));
    return parts.join(' · ');
  }

  function toggle(s: Source) {
    store.setActive(store.activeSource === s.id ? null : s.id);
  }
</script>

<div class="sources">
  <HudPanel caption={t('Sources')} trailing={String(store.sources.length)}>
    <div class="rows">
      {#if !store.sources.length}<p class="empty">{t('No source yet.')}</p>{/if}
      {#each store.sources as s (s.id)}
        {@const kind = kindLabel(s.kind)}
        {@const cat = store.catalogs[s.id]}
        <div
          class="row"
          class:on={store.activeSource === s.id}
          role="button"
          tabindex="0"
          title={t('Click to show only this source')}
          onclick={() => toggle(s)}
          onkeydown={(e) => e.key === 'Enter' && toggle(s)}
        >
          <span class="kind"><Tag text={kind} color={neon(kind)} /></span>
          <div class="text">
            <b>{s.name}</b>
            {#if store.loading.has(s.id)}
              <small class="busy">{t('Loading…')}</small>
            {:else if store.errors[s.id]}
              <small class="error" title={store.errors[s.id]}>{friendly(store.errors[s.id])}</small>
              {#if cat}<small>{t('Showing the copy from the last launch.')}</small>{/if}
            {:else}
              <small>{t('{channels} channels · {movies} movies · {series} series', { channels: cat?.channels.length ?? 0, movies: cat?.movies.length ?? 0, series: cat?.series.length ?? 0 })}</small>
            {/if}
            {#if cat?.account}
              {@const soon = cat.account.expires && cat.account.expires * 1000 - Date.now() < 14 * 86_400_000}
              <small class="account" class:warn={soon || (cat.account.status && cat.account.status !== 'Active')}>{accountLine(cat.account)}</small>
            {/if}
          </div>
          <button
            class="icon"
            title={t('Refresh')}
            onclick={(e) => {
              e.stopPropagation();
              void store.refresh(s);
            }}>&#xE72C;</button
          >
          <button
            class="icon remove"
            title={t('Remove')}
            onclick={(e) => {
              e.stopPropagation();
              store.remove(s);
            }}>&#xE74D;</button
          >
        </div>
      {/each}
    </div>
  </HudPanel>
  <div class="side">
    <AddSource />
    <div class="transfer">
      <button onclick={exportSettings} title={t('Sources, favourites, lists, hidden categories, progress: to a file')}
        ><span class="icon">&#xEDE1;</span> {t('Export settings…')}</button
      >
      <button onclick={importSettings} title={t('From a file exported on this PC or another')}><span class="icon">&#xE8B5;</span> {t('Import settings…')}</button>
    </div>
  </div>
</div>

<style>
  .sources {
    flex: 1;
    min-height: 0;
    display: grid;
    grid-template-columns: 1fr 460px;
    align-items: start;
    gap: 32px;
    padding: 16px 28px;
  }
  .side {
    display: flex;
    flex-direction: column;
    gap: 16px;
  }
  .transfer {
    display: flex;
    gap: 10px;
  }
  .transfer button {
    flex: 1;
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 8px;
    height: 39px;
    box-sizing: border-box;
    padding: 0 12px;
    border-radius: 999px;
    border: 1px solid rgb(233 236 242 / 0.1);
    font-size: 15px;
    color: var(--cyan);
  }
  .transfer button:hover {
    border-color: var(--cyan);
  }
  .rows {
    overflow-y: auto;
  }
  .empty {
    margin: 0;
    padding: 24px;
    font-size: 15px;
    color: var(--dim);
  }
  .row {
    position: relative;
    display: flex;
    align-items: center;
    gap: 14px;
    padding: 14px 18px;
    border-bottom: 1px solid rgb(233 236 242 / 0.03);
    cursor: pointer;
  }
  .row:hover {
    background: rgb(233 236 242 / 0.03);
  }
  .row.on {
    background: rgb(143 168 255 / 0.12);
  }
  .row.on::before {
    content: '';
    position: absolute;
    left: 0;
    top: 10px;
    bottom: 10px;
    width: 3px;
    border-radius: 2px;
    background: var(--magenta);
  }
  /* One width for M3U, Xtream and Stalker, so the names line up. */
  .kind {
    flex: none;
    width: 58px;
    display: flex;
  }
  .text {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 3px;
  }
  .text b {
    font-size: 19px;
    font-weight: 600;
  }
  .text small {
    font-size: 14px;
    color: var(--dim);
  }
  .text .busy {
    color: var(--acid);
  }
  .text .error {
    color: var(--magenta);
  }
  .text .account {
    color: var(--cyan);
  }
  .text .account.warn {
    color: var(--acid);
  }
  .row button {
    font-size: 14px;
    padding: 6px;
    color: var(--dim);
  }
  .row button:hover {
    color: var(--text);
  }
  .row .remove:hover {
    color: var(--magenta);
  }
</style>
