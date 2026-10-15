<script lang="ts">
  import type { Snippet } from "svelte";
  import Icon, { type IconName } from "../Icon.svelte";

  /// One titled card on a settings page. Rows (`SettingRow`) stack inside it,
  /// separated by hairlines; `actions` sits at the header's right edge (a
  /// master switch, a reset button).
  let {
    id,
    title,
    description,
    icon,
    actions,
    children,
  }: {
    /// Search target for a whole card (no single row to point at).
    id?: string;
    title: string;
    description?: string;
    icon?: IconName;
    actions?: Snippet;
    children: Snippet;
  } = $props();

  const headingId = $props.id();
</script>

<section class="set-section" aria-labelledby={headingId} data-setting={id}>
  <header>
    {#if icon}
      <span class="chip" aria-hidden="true"><Icon name={icon} size={16} /></span>
    {/if}
    <div class="titles">
      <h3 id={headingId}>{title}</h3>
      {#if description}<p>{description}</p>{/if}
    </div>
    {#if actions}<div class="actions">{@render actions()}</div>{/if}
  </header>
  <div class="body">{@render children()}</div>
</section>

<style>
  /* A flex column so the last card of a column can stretch to the column's
     bottom edge: the body grows and its rows share the extra height evenly
     (see `.body > :global(.set-row)`), so there is never a blank strip. */
  .set-section {
    position: relative;
    display: flex;
    flex-direction: column;
    margin: 0 0 20px;
    border: 1px solid var(--line);
    border-radius: var(--radius-card);
    background: var(--surface-1);
  }
  header {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 14px 18px;
    border-bottom: 1px solid var(--line);
  }
  .chip {
    display: grid;
    flex: none;
    place-items: center;
    width: 30px;
    height: 30px;
    border-radius: var(--radius-control);
    background: color-mix(in srgb, var(--accent) 14%, transparent);
    color: var(--accent);
  }
  .titles {
    flex: 1 1 auto;
    min-width: 0;
  }
  h3 {
    margin: 0;
    font-size: 15px;
    font-weight: 650;
    line-height: 1.3;
  }
  .titles p {
    margin: 2px 0 0;
    color: var(--text-muted);
    font-size: 12.5px;
    line-height: 1.45;
  }
  .actions {
    display: flex;
    flex: none;
    align-items: center;
    gap: 8px;
  }
  .set-section:global([data-found])::after {
    content: "";
    position: absolute;
    inset: 0;
    border-radius: inherit;
    box-shadow: inset 0 0 0 2px var(--accent);
    pointer-events: none;
    animation: found 1.6s var(--ease-enter) forwards;
  }
  @keyframes found {
    0%,
    35% {
      opacity: 1;
    }
    100% {
      opacity: 0;
    }
  }
  .body {
    display: flex;
    flex: 1 1 auto;
    flex-direction: column;
  }
  .body > :global(.set-row) {
    flex: 1 1 auto;
  }
  /* Header-less rows (a section with no body content) never leave a stray rule. */
  .body:empty {
    display: none;
  }
  header:has(+ .body:empty) {
    border-bottom: 0;
  }
</style>
