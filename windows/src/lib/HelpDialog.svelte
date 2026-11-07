<script lang="ts">
  import { panels } from './dialogs.svelte';
  import Modal from './Modal.svelte';

  /** `?` or F1: every shortcut, by screen. */
  const GROUPS: [string, [string, string][]][] = [
    [
      'Everywhere',
      [
        ['Ctrl 1…6', 'Home, Live, Guide, Movies, Series, Sources'],
        ['Ctrl K', 'Search'],
        ['Ctrl + / − / 0', 'Interface size'],
        ['F11', 'Fullscreen window'],
        ['+ / − / M', 'Volume, mute'],
        ['Ctrl .', 'Stop playback'],
        ['? or F1', 'This help'],
      ],
    ],
    [
      'Live',
      [
        ['↑ ↓', 'Previous, next channel'],
        ['← →', 'Previous, next tab'],
        ['0–9', 'Channel number (in a favourites tab: its position)'],
        ['Enter', 'Fullscreen'],
        ['Space', 'Pause'],
        ['S', 'Favourite'],
        ['Alt ↑ ↓', 'Move in the favourites list'],
        ['R', 'Record'],
        ['P', 'Mini player'],
        ['Esc', 'Stop'],
      ],
    ],
    [
      'Fullscreen player',
      [
        ['↑ ↓', 'Zap'],
        ['← →', '−10 s, +10 s (live: within the buffer)'],
        ['L', 'Back to live'],
        ['N', 'Next episode'],
        ['R / P', 'Record, mini player'],
        ['Esc or F', 'Back to the list'],
      ],
    ],
    [
      'Guide',
      [
        ['↑ ↓ ← →', 'Move between programmes'],
        ['Enter', 'Watch, replay or set a reminder'],
        ['Page ↑ ↓', 'Earlier, later (an hour)'],
        ['Home', 'Back to now'],
      ],
    ],
    [
      'Gamepad',
      [
        ['D-pad', 'Arrows'],
        ['A / B', 'Enter, back'],
        ['X / Y', 'Pause, fullscreen'],
        ['LB / RB', 'Previous, next tab'],
        ['Start', 'Search'],
      ],
    ],
  ];
</script>

<Modal title="Keyboard shortcuts" width={820} close={() => (panels.help = false)}>
  <div class="groups">
    {#each GROUPS as [title, keys] (title)}
      <section>
        <h3 class="mono">{title}</h3>
        <dl>
          {#each keys as [key, what] (key)}
            <dt><kbd class="mono">{key}</kbd></dt>
            <dd>{what}</dd>
          {/each}
        </dl>
      </section>
    {/each}
  </div>
</Modal>

<style>
  .groups {
    columns: 2;
    column-gap: 36px;
  }
  section {
    break-inside: avoid;
    margin-bottom: 20px;
  }
  h3 {
    margin: 0 0 8px;
    font-size: 12px;
    font-weight: 500;
    color: var(--acid);
  }
  dl {
    display: grid;
    grid-template-columns: auto 1fr;
    gap: 6px 14px;
    margin: 0;
  }
  dt {
    white-space: nowrap;
  }
  kbd {
    padding: 2px 7px;
    border-radius: 5px;
    border: 1px solid rgb(243 238 255 / 0.25);
    font-size: 12px;
  }
  dd {
    margin: 0;
    font-size: 15px;
    color: var(--dim);
  }
</style>
