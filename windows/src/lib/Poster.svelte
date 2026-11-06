<script lang="ts">
  import Tag from './Tag.svelte';
  import { neon } from './theme';

  /** Poster card; lights up in its neon on hover and keyboard focus. */
  let { title, url, rating, onclick }: { title: string; url: string | null; rating: number | null; onclick: () => void } = $props();
  let failed = $state(false);
</script>

<button class="poster" style:--c={neon(title)} {onclick} {title}>
  <span class="art">
    {#if url && !failed}
      <img src={url} alt="" loading="lazy" decoding="async" referrerpolicy="no-referrer" onerror={() => (failed = true)} />
    {:else}
      <span class="fallback">{title}</span>
    {/if}
  </span>
  <span class="caption">
    <b>{title}</b>
    {#if rating && rating > 0}<Tag text={rating.toFixed(1)} />{/if}
  </span>
</button>

<style>
  .poster {
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
    border-radius: 6px;
    overflow: hidden;
    background: linear-gradient(to bottom, var(--night), var(--ink));
    outline: 1px solid rgb(243 238 255 / 0.12);
    outline-offset: -1px;
    transition: transform 0.3s cubic-bezier(0.3, 1.4, 0.5, 1), box-shadow 0.3s, outline-color 0.2s;
  }
  .poster:hover .art,
  .poster:focus-visible .art {
    transform: scale(1.03);
    outline: 2px solid var(--c);
    box-shadow: 0 0 18px color-mix(in srgb, var(--c) 55%, transparent);
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
