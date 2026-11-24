<script lang="ts">
  import { t } from './i18n.svelte';
  import Menu from './Menu.svelte';
  import { player } from './player.svelte';
  import { clock } from './theme';
  import { qualityLabel, trackName } from './tracks';

  const chapters = $derived(player.chapters.map((c, i) => ({ id: i, name: `${clock(c.start)} · ${c.title}`, selected: i === player.chapter })));

  const video = $derived(player.videoTracks.map((tr) => ({ id: tr.id, name: qualityLabel(tr) || t('Track {n}', { n: tr.id }), selected: tr.selected })));
  const audio = $derived(player.audioTracks.map((tr) => ({ id: tr.id, name: trackName(tr) || t('Track {n}', { n: tr.id }), selected: tr.selected })));
  const subs = $derived([
    { id: 'no' as const, name: t('Off'), selected: !player.subtitleTracks.some((tr) => tr.selected) },
    ...player.subtitleTracks.map((tr) => ({ id: tr.id, name: trackName(tr) || t('Track {n}', { n: tr.id }), selected: tr.selected })),
  ]);
</script>

{#if chapters.length}
  <Menu label={t('Chapters')} items={chapters} onpick={(i) => player.seekTo(player.chapters[i].start)} />
{/if}
{#if video.length > 1}
  <Menu label={t('Quality')} items={video} onpick={(id) => player.selectTrack('video', id)} />
{/if}
{#if audio.length > 1}
  <Menu label={t('Audio')} items={audio} onpick={(id) => player.selectTrack('audio', id)} />
{/if}
{#if subs.length > 1}
  <Menu label={t('Subtitles')} items={subs} onpick={(id: number | 'no') => player.selectTrack('sub', id)} />
{/if}
