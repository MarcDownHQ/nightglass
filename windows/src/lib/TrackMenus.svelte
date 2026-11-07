<script lang="ts">
  import { t } from './i18n.svelte';
  import Menu from './Menu.svelte';
  import { player, trackName } from './player.svelte';

  const audio = $derived(player.audioTracks.map((tr) => ({ id: tr.id, name: trackName(tr) || t('Track {n}', { n: tr.id }), selected: tr.selected })));
  const subs = $derived([
    { id: 'no' as const, name: t('Off'), selected: !player.subtitleTracks.some((tr) => tr.selected) },
    ...player.subtitleTracks.map((tr) => ({ id: tr.id, name: trackName(tr) || t('Track {n}', { n: tr.id }), selected: tr.selected })),
  ]);
</script>

{#if audio.length > 1}
  <Menu label={t('Audio')} items={audio} onpick={(id) => player.selectTrack('audio', id)} />
{/if}
{#if subs.length > 1}
  <Menu label={t('Subtitles')} items={subs} onpick={(id: number | 'no') => player.selectTrack('sub', id)} />
{/if}
