<script lang="ts">
  import { t } from './i18n.svelte';
  import { dialogFocus, prompt } from './dialogs.svelte';

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
    <div class="dialog" role="dialog" aria-label={prompt.title} use:dialogFocus>
      <h2>{prompt.title}</h2>
      <!-- svelte-ignore a11y_autofocus -->
      <input bind:this={input} bind:value={prompt.value} type={prompt.secret ? 'password' : 'text'} {onkeydown} autofocus spellcheck="false" />
      <div class="buttons">
        <button onclick={() => prompt.answer(null)}>{t('Cancel')}</button>
        <button class="ok" onclick={() => prompt.answer(prompt.value)}>{t('OK')}</button>
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
    /* Centred on a whole pixel (see .dialog): a half-pixel offset blurs text and moves it off centre. */
    place-items: start center;
    padding-top: round(down, 50vh, 1px);
    background: rgb(10 12 17 / 0.6);
  }
  .dialog {
    translate: 0 round(down, -50%, 1px);
    width: 420px;
    display: flex;
    flex-direction: column;
    gap: 16px;
    padding: 22px;
    border-radius: 14px;
    background: var(--ink);
    border: 1px solid rgb(233 236 242 / 0.1);
  }
  h2 {
    margin: 0;
    font-size: 20px;
  }
  input {
    padding: 10px 12px;
    border-radius: 8px;
    border: 1px solid rgb(233 236 242 / 0.2);
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
