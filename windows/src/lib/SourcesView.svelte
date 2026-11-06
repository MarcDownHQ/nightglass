<script lang="ts">
  import AddSource from './AddSource.svelte';
  import HudPanel from './HudPanel.svelte';
  import { kindLabel, type Source } from './models';
  import { store } from './store.svelte';
  import Tag from './Tag.svelte';
  import { neon } from './theme';

  function toggle(s: Source) {
    store.setActive(store.activeSource === s.id ? null : s.id);
  }
</script>

<div class="sources">
  <HudPanel caption="Sources" trailing={String(store.sources.length)}>
    <div class="rows">
      {#if !store.sources.length}<p class="empty">No source yet.</p>{/if}
      {#each store.sources as s (s.id)}
        {@const kind = kindLabel(s.kind)}
        {@const cat = store.catalogs[s.id]}
        <div
          class="row"
          class:on={store.activeSource === s.id}
          role="button"
          tabindex="0"
          title="Click to show only this source"
          onclick={() => toggle(s)}
          onkeydown={(e) => e.key === 'Enter' && toggle(s)}
        >
          <Tag text={kind} color={neon(kind)} />
          <div class="text">
            <b>{s.name}</b>
            {#if store.loading.has(s.id)}
              <small class="busy">Loading…</small>
            {:else if store.errors[s.id]}
              <small class="error">{store.errors[s.id]}</small>
            {:else}
              <small>{cat?.channels.length ?? 0} channels · {cat?.movies.length ?? 0} movies · {cat?.series.length ?? 0} series</small>
            {/if}
          </div>
          <button
            class="icon"
            title="Refresh"
            onclick={(e) => {
              e.stopPropagation();
              void store.refresh(s);
            }}>&#xE72C;</button
          >
          <button
            class="icon remove"
            title="Remove"
            onclick={(e) => {
              e.stopPropagation();
              store.remove(s);
            }}>&#xE74D;</button
          >
        </div>
      {/each}
    </div>
  </HudPanel>
  <AddSource />
</div>

<style>
  .sources {
    flex: 1;
    min-height: 0;
    display: grid;
    grid-template-columns: 1fr 460px;
    align-items: start;
    gap: 32px;
    padding: 16px 28px;
  }
  .rows {
    overflow-y: auto;
  }
  .empty {
    margin: 0;
    padding: 24px;
    font-size: 15px;
    color: var(--dim);
  }
  .row {
    position: relative;
    display: flex;
    align-items: center;
    gap: 14px;
    padding: 14px 18px;
    border-bottom: 1px solid rgb(5 217 232 / 0.08);
    cursor: pointer;
  }
  .row:hover {
    background: rgb(243 238 255 / 0.03);
  }
  .row.on {
    background: rgb(255 42 109 / 0.12);
  }
  .row.on::before {
    content: '';
    position: absolute;
    left: 0;
    top: 10px;
    bottom: 10px;
    width: 3px;
    border-radius: 2px;
    background: var(--magenta);
    box-shadow: 0 0 6px var(--magenta);
  }
  .text {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 3px;
  }
  .text b {
    font-size: 19px;
    font-weight: 600;
  }
  .text small {
    font-size: 14px;
    color: var(--dim);
  }
  .text .busy {
    color: var(--acid);
  }
  .text .error {
    color: var(--magenta);
  }
  .row button {
    font-size: 14px;
    padding: 6px;
    color: var(--dim);
  }
  .row button:hover {
    color: var(--text);
  }
  .row .remove:hover {
    color: var(--magenta);
  }
</style>
