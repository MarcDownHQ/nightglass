<script lang="ts">
  import type { Channel } from './models';
  import { initials } from './theme';

  let { channel, color, size = 42 }: { channel: Channel; color: string; size?: number } = $props();
  let failed = $state(false);
</script>

<div class="logo" style:width="{size}px" style:height="{size}px" style:--c={color}>
  {#if channel.logo && !failed}
    <img src={channel.logo} alt="" loading="lazy" referrerpolicy="no-referrer" onerror={() => (failed = true)} />
  {:else}
    <span style:font-size="{size * 0.36}px">{initials(channel.name)}</span>
  {/if}
</div>

<style>
  .logo {
    flex: none;
    display: grid;
    place-items: center;
    box-sizing: border-box;
    border-radius: 7px;
    background: rgb(5 1 15 / 0.75);
    border: 1.5px solid var(--c);
    box-shadow: 0 0 6px color-mix(in srgb, var(--c) 60%, transparent);
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
    font-weight: 800;
    color: #fff;
    text-shadow: 0 0 6px var(--c);
  }
</style>
