<script lang="ts">
  import { prompt } from './dialogs.svelte';

  let input: HTMLInputElement | undefined = $state();

  $effect(() => {
    if (prompt.title) input?.select();
  });

  function onkeydown(e: KeyboardEvent) {
    if (e.key === 'Escape') prompt.answer(null);
    else if (e.key === 'Enter') prompt.answer(prompt.value);
    else return;
    e.preventDefault();
    e.stopPropagation();
  }
</script>

{#if prompt.title}
  <div class="scrim" role="presentation" onmousedown={(e) => e.target === e.currentTarget && prompt.answer(null)}>
    <div class="dialog" role="dialog" aria-label={prompt.title}>
      <h2>{prompt.title}</h2>
      <!-- svelte-ignore a11y_autofocus -->
      <input bind:this={input} bind:value={prompt.value} {onkeydown} autofocus spellcheck="false" />
      <div class="buttons">
        <button onclick={() => prompt.answer(null)}>Cancel</button>
        <button class="ok" onclick={() => prompt.answer(prompt.value)}>OK</button>
      </div>
    </div>
  </div>
{/if}

<style>
  .scrim {
    position: fixed;
    inset: 0;
    z-index: 110;
    display: grid;
    place-items: center;
    background: rgb(5 1 15 / 0.6);
  }
  .dialog {
    width: 420px;
    display: flex;
    flex-direction: column;
    gap: 16px;
    padding: 22px;
    border-radius: 14px;
    background: var(--ink);
    border: 1px solid rgb(5 217 232 / 0.4);
    box-shadow: 0 0 24px rgb(5 217 232 / 0.2);
  }
  h2 {
    margin: 0;
    font-size: 20px;
  }
  input {
    padding: 10px 12px;
    border-radius: 8px;
    border: 1px solid rgb(243 238 255 / 0.2);
    background: var(--night);
    color: var(--text);
    font: inherit;
    font-size: 16px;
  }
  input:focus {
    outline: none;
    border-color: var(--cyan);
  }
  .buttons {
    display: flex;
    justify-content: flex-end;
    gap: 10px;
  }
  .buttons button {
    padding: 7px 16px;
    border-radius: 999px;
    font-size: 15px;
    font-weight: 600;
    color: var(--dim);
  }
  .buttons .ok {
    background: var(--cyan);
    color: var(--ink);
  }
</style>
