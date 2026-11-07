<script lang="ts">
  import { t } from './i18n.svelte';
  import HudPanel from './HudPanel.svelte';
  import type { LiveItem } from './models';
  import NeonText from './NeonText.svelte';
  import { player } from './player.svelte';
  import ProgressLine from './ProgressLine.svelte';
  import SignalBars from './SignalBars.svelte';
  import { store } from './store.svelte';
  import { displayName, hm, progress } from './theme';
  import TrackMenus from './TrackMenus.svelte';

  let { item }: { item?: LiveItem } = $props();
  const np = $derived(item ? store.nowNext(item.channel) : {});
</script>

<HudPanel>
  <div class="top">
    <div class="now">
      <span class="label mono">{t('Now')}{np.now ? ` · ${hm(np.now.start)}–${hm(np.now.end)}` : ''}</span>
      <NeonText text={np.now?.title ?? (item ? displayName(item.channel.name) : '—')} size={28} weight={700} lines={2} />
      {#if np.now?.desc}<p class="desc">{np.now.desc}</p>{/if}
      <div class="progress"><ProgressLine value={np.now ? progress(np.now, store.now) : 0} height={3} /></div>
    </div>
    <div class="next">
      <span class="label mono">{t('Next')}{np.next ? ` · ${hm(np.next.start)}` : ''}</span>
      <b>{np.next?.title ?? '—'}</b>
    </div>
  </div>
  {#if item}<footer class="mono">
    {#if item.channel.number}<span>{t('Ch {n}', { n: item.channel.number })}</span>{/if}
    {#if player.height > 0}<span>{player.height}p</span>{/if}
    <span>{item.source.name}</span>
    <span class="spacer"></span>
    <TrackMenus />
    <SignalBars level={player.signal} />
  </footer>{/if}
</HudPanel>

<style>
  .top {
    display: flex;
  }
  .now {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding: 18px;
  }
  .next {
    flex: none;
    width: 240px;
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding: 18px;
    border-left: 1px solid rgb(5 217 232 / 0.14);
  }
  .label {
    font-size: 11px;
    font-weight: 500;
    color: var(--acid);
  }
  .next .label {
    color: var(--cyan);
  }
  /* Its first line centred on the first line of the bigger title beside it. */
  .next b {
    margin-top: 4px;
    font-size: 19px;
    font-weight: 600;
    display: -webkit-box;
    -webkit-box-orient: vertical;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    overflow: hidden;
  }
  .desc {
    margin: 0;
    font-size: 15px;
    line-height: 1.35;
    color: var(--dim);
    display: -webkit-box;
    -webkit-box-orient: vertical;
    -webkit-line-clamp: 3;
    line-clamp: 3;
    overflow: hidden;
  }
  .progress {
    padding-top: 6px;
  }
  footer {
    display: flex;
    align-items: center;
    gap: 14px;
    min-height: 38px;
    padding: 0 18px;
    border-top: 1px solid rgb(5 217 232 / 0.14);
    font-size: 11px;
    color: var(--dim);
  }
  footer :global(.trigger) {
    font-family: var(--title);
    font-size: 14px;
    color: var(--text);
  }
  .spacer {
    flex: 1;
  }
</style>
