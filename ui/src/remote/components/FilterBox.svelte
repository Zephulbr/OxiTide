<script lang="ts">
  import Icon from "../../lib/components/Icon.svelte";
  import { t } from "../i18n.svelte";

  /// The in-list filter box (track lists, Library grids). `label` is the
  /// placeholder and accessible name.
  let { value = $bindable(""), label }: { value?: string; label: string } = $props();
</script>

<div class="filter" role="search">
  <Icon name="search" size={18} />
  <input bind:value type="search" placeholder={label} aria-label={label} enterkeyhint="search" autocomplete="off" />
  {#if value}
    <button type="button" class="icon-btn" aria-label={t("filter.clear")} onclick={() => (value = "")}>
      <Icon name="x" size={18} />
    </button>
  {/if}
</div>

<style>
  .filter {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 0 4px 0 14px;
    margin: 0 0 12px;
    border: 1px solid var(--line);
    border-radius: 999px;
    background: var(--surface-float);
    backdrop-filter: blur(var(--glass-blur));
    -webkit-backdrop-filter: blur(var(--glass-blur));
    color: var(--text-muted);
  }
  input {
    flex: 1;
    min-width: 0;
    height: 44px;
    border: none;
    background: none;
    color: var(--text);
    font-size: 16px;
    outline: none;
  }
  input::-webkit-search-cancel-button {
    display: none;
  }
</style>
