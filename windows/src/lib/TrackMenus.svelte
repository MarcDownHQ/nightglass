<script lang="ts">
  import Menu from './Menu.svelte';
  import { player, trackName } from './player.svelte';

  const audio = $derived(player.audioTracks.map((t) => ({ id: t.id, name: trackName(t) || `Track ${t.id}`, selected: t.selected })));
  const subs = $derived([
    { id: 'no' as const, name: 'Off', selected: !player.subtitleTracks.some((t) => t.selected) },
    ...player.subtitleTracks.map((t) => ({ id: t.id, name: trackName(t) || `Track ${t.id}`, selected: t.selected })),
  ]);
</script>

{#if audio.length > 1}
  <Menu label="Audio" items={audio} onpick={(id) => player.selectTrack('audio', id)} />
{/if}
{#if subs.length > 1}
  <Menu label="Subtitles" items={subs} onpick={(id: number | 'no') => player.selectTrack('sub', id)} />
{/if}
