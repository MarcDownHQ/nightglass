<script lang="ts">
  import { t } from './i18n.svelte';
  import { organizer, prompt } from './dialogs.svelte';
  import { newList } from './channelMenu';
  import { library, listLabel } from './library.svelte';
  import { categoryKey, categoryLabel } from './lineup';
  import { removePin, toggleCategoryLock, unlock } from './parental';
  import { store } from './store.svelte';

  /** Categories (order, names, hidden) and favourites lists, in one window. */
  let dragged = $state<string | null>(null);

  const categories = $derived(store.allCategories);

  /** Saves the whole order as shown, with `key` moved before `before` (null: to the end). */
  function place(key: string, before: string | null) {
    const keys = categories.map((c) => c.key).filter((k) => k !== key);
    keys.splice(before === null ? keys.length : Math.max(0, keys.indexOf(before)), 0, key);
    library.categoryOrder = keys;
    library.persist('categoryOrder');
  }

  function step(key: string, delta: number) {
    const keys = categories.map((c) => c.key);
    const i = keys.indexOf(key);
    const j = i + delta;
    if (j < 0 || j >= keys.length) return;
    [keys[i], keys[j]] = [keys[j], keys[i]];
    library.categoryOrder = keys;
    library.persist('categoryOrder');
  }

  function rename(key: string, original: string, value: string) {
    const name = value.trim();
    if (!name || name === original) delete library.categoryNames[key];
    else library.categoryNames[key] = name;
    library.persist('categoryNames');
  }

  /** The name the playlist gives, to tell a renamed category from its source. */
  function original(key: string): string {
    const group = store.allChannels.find((x) => x.channel.group && categoryKey(x.channel.group) === key)?.channel.group;
    return group ? categoryLabel(group) : key;
  }

  function reset() {
    library.categoryOrder = [];
    library.categoryNames = {};
    library.persist('categoryOrder');
    library.persist('categoryNames');
  }

  function onkeydown(e: KeyboardEvent) {
    if (organizer.open && e.key === 'Escape') {
      organizer.open = false;
      e.preventDefault();
      e.stopPropagation();
    }
  }
</script>

<svelte:window onkeydowncapture={onkeydown} />

{#if organizer.open}
  <div class="scrim" role="presentation" onmousedown={(e) => e.target === e.currentTarget && (organizer.open = false)}>
    <div class="dialog" role="dialog" aria-label={t('Organize')}>
      <header>
        <h2>{t('Organize')}</h2>
        <button class="icon close" title={t('Close (Esc)')} onclick={() => (organizer.open = false)}>&#xE8BB;</button>
      </header>

      <div class="columns">
        <section>
          <div class="title">
            <h3 class="mono">{t('Categories')}</h3>
            <button class="link" onclick={reset}>{t('Reset order and names')}</button>
          </div>
          <p class="hint">{t('Drag to reorder. The same name in several sources, with or without a country prefix, is one category.')}</p>
          <ul class="list">
            {#each categories as c, i (c.key)}
              {@const hidden = library.hiddenGroups.has(c.key)}
              <li
                class:hidden
                class:dragging={dragged === c.key}
                draggable="true"
                ondragstart={(e) => {
                  dragged = c.key;
                  e.dataTransfer?.setData('text/plain', c.key);
                }}
                ondragend={() => (dragged = null)}
                ondragover={(e) => e.preventDefault()}
                ondrop={(e) => {
                  e.preventDefault();
                  if (dragged && dragged !== c.key) place(dragged, c.key);
                  dragged = null;
                }}
              >
                <span class="icon grip">&#xE76F;</span>
                <input value={c.label} aria-label={t('Name of {name}', { name: c.label })} onchange={(e) => rename(c.key, original(c.key), e.currentTarget.value)} />
                <button class="icon" title={t('Up')} disabled={i === 0} onclick={() => step(c.key, -1)}>&#xE70E;</button>
                <button class="icon" title={t('Down')} disabled={i === categories.length - 1} onclick={() => step(c.key, 1)}>&#xE70D;</button>
                <button class="icon eye" title={hidden ? t('Show') : t('Hide')} onclick={() => library.toggleHidden(library.hiddenGroups, c.key)}>
                  {hidden ? '' : ''}
                </button>
                <button
                  class="icon lock"
                  class:on={library.lockedGroups.has(c.key)}
                  title={library.lockedGroups.has(c.key) ? t('Behind the parental PIN') : t('Put behind the parental PIN')}
                  onclick={() => toggleCategoryLock(c.key)}
                >
                  {library.lockedGroups.has(c.key) ? '' : ''}
                </button>
              </li>
            {:else}
              <li class="empty">{t('No categories.')}</li>
            {/each}
          </ul>
        </section>

        <section>
          <div class="title">
            <h3 class="mono">{t('Favorites lists')}</h3>
            <button class="link" onclick={() => newList()}>{t('New list')}</button>
          </div>
          <p class="hint">{t('Each list is a tab of its own, numbered from 1. Right-click a channel to add it; drag it in its tab to reorder.')}</p>
          <ul class="list">
            {#each library.favoriteLists as l, i (l.id)}
              <li>
                <span class="icon star">&#xE735;</span>
                <input value={listLabel(l)} aria-label={t('Name of {name}', { name: listLabel(l) })} onchange={(e) => library.renameList(l.id, e.currentTarget.value)} />
                <span class="count mono">{l.items.length}</span>
                <!-- The first list can't be deleted: its button keeps its place, hidden, so the counts line up. -->
                <button class="icon" class:placeholder={i === 0} disabled={i === 0} title={t('Delete the list')} onclick={() => library.deleteList(l.id)}>&#xE74D;</button>
              </li>
            {/each}
          </ul>

          <div class="title pin">
            <h3 class="mono">{t('Parental PIN')}</h3>
          </div>
          <p class="hint">
            {#if library.pinHash}
              {t('Locked categories (live, movies and series) stay out of sight until the PIN is typed, with the lock in the title bar.')}
            {:else}
              {t('No PIN yet. The lock next to a category makes one.')}
            {/if}
          </p>
          {#if library.pinHash}
            <div class="pin-buttons">
              <button
                class="link"
                onclick={async () => {
                  if (!(await unlock())) return;
                  const pin = await prompt.ask(t('New parental PIN'), '', { secret: true });
                  if (pin) await library.setPin(pin);
                }}>{t('Change the PIN')}</button
              >
              <button class="link" onclick={() => removePin()}>{t('Remove the PIN')}</button>
            </div>
          {/if}
        </section>
      </div>
    </div>
  </div>
{/if}

<style>
  .scrim {
    position: fixed;
    inset: 0;
    z-index: 105;
    display: grid;
    /* Centred on a whole pixel (see .dialog): a half-pixel offset blurs text and moves it off centre. */
    place-items: start center;
    padding-top: round(down, 50vh, 1px);
    background: rgb(10 12 17 / 0.6);
  }
  .dialog {
    translate: 0 round(down, -50%, 1px);
    width: min(980px, calc(100vw - 80px));
    max-height: calc(100vh - 120px);
    display: flex;
    flex-direction: column;
    gap: 14px;
    padding: 22px 24px;
    border-radius: 14px;
    background: var(--ink);
    border: 1px solid rgb(233 236 242 / 0.1);
  }
  header {
    display: flex;
    align-items: center;
  }
  h2 {
    flex: 1;
    margin: 0;
    font-size: 24px;
  }
  .close {
    padding: 8px;
    font-size: 12px;
    color: var(--dim);
  }
  .columns {
    min-height: 0;
    display: grid;
    grid-template-columns: 1.3fr 1fr;
    gap: 28px;
  }
  section {
    min-height: 0;
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .title {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
  }
  h3 {
    margin: 0;
    font-size: 12px;
    font-weight: 500;
    color: var(--acid);
  }
  .link {
    font-size: 14px;
    line-height: 18px;
    color: var(--cyan);
  }
  .hint {
    margin: 0;
    font-size: 13px;
    color: var(--dim);
  }
  .list {
    min-height: 0;
    max-height: 56vh;
    overflow-y: auto;
    margin: 0;
    padding: 0;
    list-style: none;
    scrollbar-width: thin;
    scrollbar-color: rgb(233 236 242 / 0.15) transparent;
  }
  li {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 4px 6px;
    border-radius: 8px;
  }
  li:hover {
    background: rgb(233 236 242 / 0.04);
  }
  li.dragging {
    opacity: 0.4;
  }
  li.hidden input {
    color: var(--dim);
    text-decoration: line-through;
  }
  li.empty {
    color: var(--dim);
    font-size: 14px;
  }
  .grip {
    cursor: grab;
    font-size: 12px;
    color: var(--dim);
  }
  .star {
    font-size: 12px;
    color: var(--acid);
  }
  input {
    flex: 1;
    min-width: 0;
    padding: 6px 8px;
    border-radius: 6px;
    border: 1px solid transparent;
    background: transparent;
    color: var(--text);
    font: inherit;
    font-size: 15px;
  }
  input:hover {
    border-color: rgb(233 236 242 / 0.12);
  }
  input:focus {
    outline: none;
    border-color: var(--cyan);
    background: var(--night);
  }
  /* One square for every row button, whatever its glyph. */
  li button {
    flex: none;
    width: 24px;
    height: 24px;
    display: grid;
    place-items: center;
    font-size: 12px;
    color: var(--dim);
  }
  li button.placeholder {
    visibility: hidden;
  }
  li button:hover:not(:disabled) {
    color: var(--text);
  }
  li button:disabled {
    opacity: 0.25;
  }
  .eye {
    font-size: 14px;
  }
  .lock.on {
    color: var(--acid);
  }
  .pin {
    margin-top: 18px;
  }
  .pin-buttons {
    display: flex;
    gap: 18px;
  }
  .count {
    font-size: 12px;
    color: var(--dim);
  }
</style>
