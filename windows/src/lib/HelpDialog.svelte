<script lang="ts">
  import { i18n, t } from './i18n.svelte';
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
        ['T', 'Cinema mode'],
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

  /** Key names as a French keyboard prints them. */
  const FRENCH_KEYS: Record<string, string> = { Enter: 'Entrée', Space: 'Espace', Esc: 'Échap', Home: 'Début', 'D-pad': 'Croix' };

  function keyLabel(key: string): string {
    // Through t() as well: on macOS, Ctrl reads ⌘ and Alt ⌥.
    return t(
      key
        .split(' ')
        .map((w) => (w === 'or' ? t('or') : i18n.lang === 'fr' ? (FRENCH_KEYS[w] ?? w) : w))
        .join(' '),
    );
  }
</script>

<Modal title={t('Keyboard shortcuts')} width={820} close={() => (panels.help = false)}>
  <div class="groups">
    {#each GROUPS as [title, keys] (title)}
      <section>
        <h3 class="mono">{t(title)}</h3>
        <dl>
          {#each keys as [key, what] (key)}
            <dt><kbd class="mono">{keyLabel(key)}</kbd></dt>
            <dd>{t(what)}</dd>
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
    font-weight: 600;
    color: var(--acid);
  }
  /* One key column for every group, so all the descriptions start at the same place. */
  dl {
    display: grid;
    grid-template-columns: 118px 1fr;
    gap: 6px 14px;
    margin: 0;
  }
  dt {
    display: flex;
    align-items: center;
    height: 20px;
    white-space: nowrap;
  }
  kbd {
    padding: 2px 7px;
    border-radius: 5px;
    border: 1px solid rgb(233 236 242 / 0.25);
    font-size: 12px;
  }
  dd {
    margin: 0;
    font-size: 15px;
    line-height: 20px;
    color: var(--dim);
  }
</style>
