<script lang="ts">
  import { errorMessage } from "../errors";
  import { t } from "../i18n.svelte";
  import Icon from "./Icon.svelte";

  let { message, onRetry }: { message: string; onRetry?: () => void } = $props();
</script>

<div class="error-state" role="alert">
  <p class="message"><Icon name="alert-triangle" size={16} /><span>{errorMessage(message)}</span></p>
  {#if onRetry}
    <button class="tool-btn" onclick={onRetry}><Icon name="rotate-cw" size={16} /> {t("common.retry")}</button>
  {/if}
</div>

<style>
  .error-state {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 12px;
  }
  .message {
    display: flex;
    align-items: flex-start;
    gap: 8px;
    margin: 0;
    color: var(--danger);
  }
  .message :global(svg) {
    flex: 0 0 auto;
    margin-top: 2px;
  }
</style>
