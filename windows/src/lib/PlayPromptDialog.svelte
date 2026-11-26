<script lang="ts">
  import { t } from './i18n.svelte';
  import Modal from './Modal.svelte';
  import { playPrompt } from './playPrompt.svelte';
  import SignInPrompt from './SignInPrompt.svelte';
</script>

{#if playPrompt.prompt}
  <Modal title={playPrompt.title} close={() => playPrompt.cancel()} width={520}>
    <div class="play-prompt">
      <SignInPrompt prompt={playPrompt.prompt} busy={playPrompt.busy} retry={() => void playPrompt.retry()} />
      <small>{t('It plays as soon as the provider lets it.')}</small>
      <div class="buttons">
        <button onclick={() => playPrompt.cancel()}>{t('Cancel')}</button>
      </div>
    </div>
  </Modal>
{/if}

<style>
  .play-prompt {
    display: flex;
    flex-direction: column;
    gap: 12px;
  }
  small {
    font-size: 13px;
    color: var(--dim);
  }
  .buttons {
    display: flex;
    justify-content: flex-end;
  }
  .buttons button {
    padding: 7px 16px;
    border-radius: 999px;
    font-size: 15px;
    font-weight: 600;
    color: var(--dim);
  }
</style>
