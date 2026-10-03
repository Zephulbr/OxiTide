<script lang="ts">
  import { openUrl } from "@tauri-apps/plugin-opener";
  import { api, type KeyringStatus } from "../api";
  import { keyringNotice } from "../keyring";
  import Icon from "./Icon.svelte";

  let status = $state<KeyringStatus | null>(null);
  const copy = $derived(keyringNotice(status));

  $effect(() => {
    void api
      .getKeyringStatus()
      .then((value) => (status = value))
      .catch(() => {});
  });

  function open(url: string) {
    openUrl(url).catch(() => {});
  }
</script>

{#if copy}
  <div class="keyring-notice" role="status">
    <Icon name="alert-triangle" size={16} />
    <div class="body">
      <p>{copy.body}</p>
      <p class="links">
        {#each copy.links as link (link.url)}
          <button class="link" type="button" onclick={() => open(link.url)}>{link.label}</button>
        {/each}
      </p>
    </div>
  </div>
{/if}

<style>
  .keyring-notice {
    display: flex;
    align-items: flex-start;
    gap: 10px;
    margin: 8px 0 12px;
    padding: 10px 12px;
    border: 1px solid color-mix(in srgb, var(--warning) 45%, transparent);
    border-radius: var(--radius-control);
    background: color-mix(in srgb, var(--warning) 8%, transparent);
    color: var(--text-muted);
  }
  .keyring-notice :global(svg) {
    flex: 0 0 auto;
    margin-top: 2px;
    color: var(--warning);
  }
  .body {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .keyring-notice p {
    margin: 0;
    line-height: 1.5;
  }
  .links {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px;
  }
  .link {
    padding: 0;
    color: var(--accent);
    text-decoration: underline;
  }
</style>
