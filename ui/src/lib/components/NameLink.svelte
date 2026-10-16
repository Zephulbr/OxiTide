<script lang="ts">
  /// A clickable name (track, album, artist) that reads as the surrounding
  /// text and underlines on hover. A span, not a button: buttons are atomic
  /// inline boxes and an overflowing one is dropped whole by text-overflow
  /// ellipsis (name-links spec NL-D5). Without `onOpen` it is plain text.
  let {
    label,
    onOpen,
    focusable = true,
    nested = false,
  }: {
    label: string;
    onOpen?: () => void;
    /// False inside rows that own a single roving tab stop.
    focusable?: boolean;
    /// Inside another control (the queue row's play button): pointer only, no
    /// link role (interactive content cannot nest), and the click stops there
    /// so the host does not also act. The host's menu is the keyboard path.
    nested?: boolean;
  } = $props();

  function onKeydown(event: KeyboardEvent) {
    if (event.key !== "Enter") return;
    event.preventDefault();
    event.stopPropagation();
    onOpen?.();
  }

  function onNestedClick(event: MouseEvent) {
    event.stopPropagation();
    onOpen?.();
  }
</script>

{#if onOpen && nested}
  <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
  <span class="name-link" title={label} onclick={onNestedClick}>{label}</span>
{:else if onOpen}
  <span
    class="name-link"
    role="link"
    tabindex={focusable ? 0 : -1}
    title={label}
    onclick={() => onOpen()}
    onkeydown={onKeydown}
  >{label}</span>
{:else}
  <span>{label}</span>
{/if}

<style>
  .name-link {
    cursor: pointer;
    text-underline-offset: 2px;
  }
  .name-link:hover {
    text-decoration: underline;
  }
  .name-link:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: 2px;
    border-radius: 2px;
  }
</style>
