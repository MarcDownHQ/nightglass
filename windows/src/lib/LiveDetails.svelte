<script lang="ts">
  import { t } from './i18n.svelte';
  import { library } from './library.svelte';
  import type { LiveItem } from './models';
  import { nav } from './nav.svelte';
  import { player } from './player.svelte';
  import SignalBars from './SignalBars.svelte';
  import { store } from './store.svelte';
  import { displayName, hm, progress, qualityTag } from './theme';
  import TrackMenus from './TrackMenus.svelte';

  /** Under the preview: what plays, what comes next, and the actions on it. */
  let { item, favorite, ontogglefavorite }: { item?: LiveItem; favorite: boolean; ontogglefavorite: () => void } = $props();

  const np = $derived(item ? store.nowNext(item.channel) : {});
  const upcoming = $derived(item ? store.programmes(item.channel).filter((p) => p.start >= (np.now?.end ?? store.now)).slice(0, 4) : []);
  const left = $derived(np.now ? Math.max(0, Math.round((np.now.end - store.now) / 60)) : 0);
  const quality = $derived(item ? qualityTag(item.channel.name) : null);

  function remind(p: { title: string; start: number; end: number }) {
    if (!item) return;
    library.toggleReminder({ channel: item.id, channelName: displayName(item.channel.name), title: p.title, start: p.start, end: p.end });
  }

  function openGuide() {
    if (item) nav.guideChannel = item.id;
    nav.section = 'guide';
  }
</script>

{#if item}
  <div class="details">
    <div class="head">
      <div class="info">
        <div class="meta">
          {#if item.channel.number}<span class="mono">{item.channel.number}</span><span>·</span>{/if}
          <b>{displayName(item.channel.name)}</b>
          {#if quality}<span>·</span><span>{quality}</span>{/if}
          {#if player.height > 0}<span>·</span><span class="mono">{player.height}p</span>{/if}
          <span>·</span><span class="source">{item.source.name}</span>
        </div>
        <h2>{np.now?.title ?? displayName(item.channel.name)}</h2>
        {#if np.now}
          <div class="when">
            <span class="mono">{hm(np.now.start)} – {hm(np.now.end)}</span>
            <span class="bar"><span style:width="{progress(np.now, store.now) * 100}%"></span></span>
            <span>{t('{n} min left', { n: left })}</span>
          </div>
        {/if}
        {#if np.now?.desc}<p class="desc">{np.now.desc}</p>{/if}
      </div>
      <div class="actions">
        <button class="chip" class:fav={favorite} aria-pressed={favorite} onclick={ontogglefavorite}>
          <span class="icon">{favorite ? '' : ''}</span>{favorite ? t('In favorites') : t('Favorite')}
        </button>
        {#if player.current?.live && player.current.item?.id === item.id}
          <button class="chip" class:recording={player.recording} onclick={() => player.toggleRecording()}>
            <span class="dot"></span>{player.recording ? t('Stop recording') : t('Record')}
          </button>
        {/if}
      </div>
    </div>

    <section class="next">
      <header>
        <h3>{t('Up next on {name}', { name: displayName(item.channel.name) })}</h3>
        <button class="link" onclick={openGuide}>{t('Full guide')}</button>
      </header>
      {#each upcoming as p (p.start)}
        {@const on = library.hasReminder(item.id, p.start)}
        <div class="prog">
          <span class="mono time">{hm(p.start)}</span>
          <span class="title">{p.title}</span>
          <button class="chip small" class:on aria-pressed={on} onclick={() => remind(p)}>
            <span class="icon">&#xEA8F;</span>{on ? t('Reminder set') : t('Remind me')}
          </button>
        </div>
      {:else}
        <p class="none">{t('No guide for this channel.')}</p>
      {/each}
    </section>

    <footer>
      <TrackMenus />
      <span class="spacer"></span>
      <SignalBars level={player.signal} />
    </footer>
  </div>
{:else}
  <p class="empty">{t('Pick a channel to see what is on.')}</p>
{/if}

<style>
  .details {
    display: flex;
    flex-direction: column;
    gap: 18px;
  }
  .head {
    display: flex;
    align-items: flex-start;
    gap: 16px;
  }
  .info {
    flex: 1;
    min-width: 0;
  }
  .meta {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 13px;
    color: var(--muted);
    white-space: nowrap;
    overflow: hidden;
  }
  .meta b {
    font-weight: 600;
    color: var(--text);
  }
  .source {
    overflow: hidden;
    text-overflow: ellipsis;
  }
  h2 {
    margin: 8px 0 0;
    font-size: 24px;
    font-weight: 700;
    letter-spacing: -0.015em;
    line-height: 1.2;
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow: hidden;
  }
  .when {
    display: flex;
    align-items: center;
    gap: 12px;
    margin-top: 12px;
    font-size: 13px;
    color: var(--muted);
  }
  .when .mono {
    color: var(--soft);
  }
  .bar {
    flex: 1;
    max-width: 240px;
    height: 4px;
    border-radius: 2px;
    background: #222836;
    overflow: hidden;
  }
  .bar span {
    display: block;
    height: 100%;
    background: var(--accent);
  }
  .desc {
    margin: 10px 0 0;
    font-size: 13px;
    line-height: 1.5;
    color: var(--muted);
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow: hidden;
  }
  .actions {
    flex: none;
    display: flex;
    gap: 8px;
  }
  .chip {
    display: inline-flex;
    align-items: center;
    gap: 7px;
    height: 34px;
    padding: 0 12px;
    border-radius: 9px;
    background: var(--raised);
    color: var(--soft);
    font-size: 13px;
    font-weight: 500;
    white-space: nowrap;
  }
  .chip:hover {
    background: var(--hover);
    color: #fff;
  }
  .chip .icon {
    font-size: 13px;
  }
  .chip.fav .icon {
    color: var(--fav);
  }
  .chip.small {
    height: 30px;
    font-size: 12px;
  }
  .chip.on {
    color: var(--accent);
  }
  .dot {
    width: 9px;
    height: 9px;
    border-radius: 50%;
    border: 2px solid var(--live);
    box-sizing: border-box;
  }
  .recording .dot {
    background: var(--live);
  }
  .next {
    padding: 8px;
    border-radius: 14px;
    background: var(--surface);
    border: 1px solid var(--border);
  }
  .next header {
    display: flex;
    align-items: center;
    padding: 8px 12px 6px;
  }
  h3 {
    flex: 1;
    margin: 0;
    font-size: 14px;
    font-weight: 600;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .link {
    font-size: 13px;
    color: var(--accent);
  }
  .link:hover {
    color: var(--accent-hover);
  }
  .prog {
    display: flex;
    align-items: center;
    gap: 14px;
    padding: 6px 12px;
    border-radius: 10px;
  }
  .prog:hover {
    background: var(--raised);
  }
  .time {
    flex: none;
    width: 48px;
    font-size: 13px;
    color: var(--muted);
  }
  .title {
    flex: 1;
    min-width: 0;
    font-size: 14px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .none,
  .empty {
    margin: 6px 12px 10px;
    font-size: 13px;
    color: var(--muted);
  }
  footer {
    display: flex;
    align-items: center;
    gap: 12px;
    font-size: 13px;
    color: var(--muted);
  }
  .spacer {
    flex: 1;
  }
</style>
