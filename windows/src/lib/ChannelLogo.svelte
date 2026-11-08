<script lang="ts">
  import type { Channel } from './models';
  import { initials } from './theme';

  /** Muted tiles for channels without a logo; a name always gets the same one. */
  const TONES = ['#26324a', '#382b47', '#21403a', '#45362a', '#2c3a4a', '#3e2b33'];

  // `color` is kept for callers; the tile takes its tone from the name.
  let { channel, size = 42 }: { channel: Channel; color?: string; size?: number } = $props();
  let failed = $state(false);
  const tone = $derived(TONES[[...channel.name].reduce((h, c) => (h * 31 + c.charCodeAt(0)) >>> 0, 7) % TONES.length]);
</script>

<div class="logo" style:width="{size}px" style:height="{size}px" style:background={channel.logo && !failed ? null : tone} style:border-radius="{Math.round(size * 0.24)}px">
  {#if channel.logo && !failed}
    <img src={channel.logo} alt="" loading="lazy" referrerpolicy="no-referrer" onerror={() => (failed = true)} />
  {:else}
    <span style:font-size="{Math.round(size * 0.3)}px">{initials(channel.name)}</span>
  {/if}
</div>

<style>
  .logo {
    flex: none;
    display: grid;
    place-items: center;
    box-sizing: border-box;
    background: var(--raised);
    overflow: hidden;
  }
  img {
    width: 100%;
    height: 100%;
    object-fit: contain;
    padding: 5px;
    box-sizing: border-box;
  }
  span {
    font-weight: 700;
    color: var(--text);
  }
</style>
