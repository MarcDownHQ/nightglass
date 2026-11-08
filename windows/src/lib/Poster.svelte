<script lang="ts">
  import { t } from './i18n.svelte';
  import Tag from './Tag.svelte';

  /** Poster card. Without artwork, a muted tile with the title; a name always gets the same tone. */
  const TONES = ['#3a2b3f', '#24403c', '#2b354f', '#4a3428', '#33303f', '#2a3f4a', '#40302f', '#283a2e'];
  let {
    title,
    url,
    rating,
    onclick,
    ondblclick,
    oncontextmenu,
    subtitle,
    saved = false,
    selected = false,
    progress = 0,
    watched = false,
  }: {
    title: string;
    url: string | null;
    rating: number | null;
    onclick: () => void;
    ondblclick?: () => void;
    /** In Ma liste. */
    saved?: boolean;
    /** Shown in the detail panel. */
    selected?: boolean;
    oncontextmenu?: (e: MouseEvent) => void;
    /** Under the title: the episode, on a Continue card. */
    subtitle?: string;
    /** Share already watched, 0…1. */
    progress?: number;
    watched?: boolean;
  } = $props();
  let failed = $state(false);
  const tone = $derived(TONES[[...title].reduce((h, c) => (h * 31 + c.charCodeAt(0)) >>> 0, 7) % TONES.length]);
</script>

<button class="poster" class:selected {onclick} {ondblclick} {oncontextmenu} {title}>
  <span class="art" style:background={url && !failed ? null : tone}>
    {#if url && !failed}
      <img src={url} alt="" loading="lazy" decoding="async" referrerpolicy="no-referrer" onerror={() => (failed = true)} />
    {:else}
      <span class="fallback">{title}</span>
    {/if}
    {#if watched}<span class="watched icon" title={t('Watched')}>&#xE73E;</span>{/if}
    {#if saved}<span class="saved icon" title={t('In my list')}>&#xEB52;</span>{/if}
    {#if progress > 0}<span class="bar"><span style:width="{progress * 100}%"></span></span>{/if}
  </span>
  <span class="caption">
    <b>{title}</b>
    {#if rating && rating > 0}<Tag text={rating.toFixed(1)} />{/if}
  </span>
  {#if subtitle}<small class="subtitle">{subtitle}</small>{/if}
</button>

<style>
  /* A flex button shrinks to its content: the poster fills its column instead. */
  .poster {
    width: 100%;
    display: flex;
    flex-direction: column;
    gap: 8px;
    text-align: left;
    min-width: 0;
  }
  .art {
    position: relative;
    aspect-ratio: 2 / 3;
    display: grid;
    place-items: center;
    border-radius: 10px;
    overflow: hidden;
    background: var(--raised);
    outline: 1px solid var(--border);
    outline-offset: -1px;
    transition: transform 0.3s cubic-bezier(0.3, 1.4, 0.5, 1), box-shadow 0.3s, outline-color 0.2s;
  }
  .poster:hover .art,
  .poster:focus-visible .art {
    transform: scale(1.02);
    outline: 1px solid rgb(255 255 255 / 0.25);
  }
  .selected .art,
  .selected:hover .art {
    outline: 2px solid var(--accent);
  }
  .poster:focus-visible {
    outline: none;
  }
  img {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    object-fit: cover;
  }
  .watched {
    position: absolute;
    top: 8px;
    right: 8px;
    display: grid;
    place-items: center;
    width: 26px;
    height: 26px;
    border-radius: 50%;
    background: rgb(10 12 17 / 0.8);
    color: var(--soft);
    font-size: 13px;
  }
  .saved {
    position: absolute;
    top: 8px;
    left: 8px;
    display: grid;
    place-items: center;
    width: 26px;
    height: 26px;
    border-radius: 7px;
    background: rgb(10 12 17 / 0.8);
    color: var(--accent);
    font-size: 12px;
  }
  .bar {
    position: absolute;
    left: 0;
    right: 0;
    bottom: 0;
    height: 4px;
    background: rgb(10 12 17 / 0.7);
  }
  .bar span {
    display: block;
    height: 100%;
    background: var(--accent);
  }
  .subtitle {
    margin-top: -4px;
    font-size: 13px;
    color: var(--dim);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .fallback {
    padding: 14px;
    font-size: 20px;
    font-weight: 700;
    text-align: center;
    overflow-wrap: anywhere;
  }
  .caption {
    display: flex;
    align-items: flex-start;
    gap: 6px;
  }
  .caption b {
    flex: 1;
    font-size: 15px;
    font-weight: 600;
    line-height: 1.25;
    display: -webkit-box;
    -webkit-box-orient: vertical;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    overflow: hidden;
  }
</style>
