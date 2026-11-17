<script lang="ts">
  import Icon from "./Icon.svelte";
  import Loading from "./Loading.svelte";
  import { searchHistory } from "../searchHistory.svelte";
  import { t } from "../i18n.svelte";

  /// The catalog search field with its recent-searches dropdown, shared by
  /// Explore (a live search) and Home (a launcher that opens Explore). It
  /// records a query in the recent searches when it is submitted (Enter, or a
  /// recent search picked) and then hands it to `onsearch`. The input keeps
  /// the `search-input` id, which `focusSearch` (Ctrl+F, /) looks up; only
  /// one page is mounted at a time.
  let {
    value,
    oninput,
    onsearch,
    busy = false,
    compact = false,
  }: {
    value: string;
    oninput: (value: string) => void;
    onsearch: (query: string) => void;
    /// A search is running over results already on screen.
    busy?: boolean;
    /// Home's smaller field; Explore's is the page's hero.
    compact?: boolean;
  } = $props();

  const history = searchHistory();

  let focused = $state(false);
  let dismissed = $state(false);
  let active = $state(-1);

  const suggestions = $derived(history.matches(value));
  const showSuggestions = $derived(focused && !dismissed && suggestions.length > 0);

  function onInput(next: string) {
    dismissed = false;
    active = -1;
    oninput(next);
  }

  function submit(query: string) {
    history.record(query);
    dismissed = true;
    active = -1;
    onsearch(query);
  }

  function onKeydown(event: KeyboardEvent) {
    if (event.key === "ArrowDown" && suggestions.length > 0) {
      event.preventDefault();
      dismissed = false;
      active = active < suggestions.length - 1 ? active + 1 : 0;
    } else if (event.key === "ArrowUp" && suggestions.length > 0) {
      event.preventDefault();
      dismissed = false;
      active = active > 0 ? active - 1 : suggestions.length - 1;
    } else if (event.key === "Enter") {
      if (showSuggestions && active >= 0) {
        event.preventDefault();
        submit(suggestions[active]);
      } else {
        submit(value);
      }
    } else if (event.key === "Escape" && showSuggestions) {
      event.preventDefault();
      dismissed = true;
      active = -1;
    }
  }

  function clearHistory() {
    history.clear();
    active = -1;
  }
</script>

<div class="search-bar" class:compact>
  <span class="search-icon" aria-hidden="true"><Icon name="search" size={16} /></span>
  <input
    id="search-input"
    name="oxitide-search"
    class="search-input search-bar-input"
    {value}
    oninput={(event) => onInput(event.currentTarget.value)}
    onkeydown={onKeydown}
    onfocus={() => {
      focused = true;
      dismissed = false;
    }}
    onblur={() => {
      focused = false;
      active = -1;
    }}
    placeholder={t("search.placeholder")}
    aria-label={t("search.placeholder")}
    autocomplete="off"
    spellcheck="false"
    role="combobox"
    aria-autocomplete="list"
    aria-expanded={showSuggestions}
    aria-controls="search-suggestions"
    aria-activedescendant={showSuggestions && active >= 0 ? `search-suggestion-${active}` : undefined}
  />
  {#if busy}
    <Loading label={t("search.searching")} />
  {/if}
  {#if showSuggestions}
    <div class="suggest">
      <div class="suggest-head">
        <span>{t("search.recent")}</span>
        <button
          type="button"
          class="suggest-clear"
          aria-label={t("search.clearRecent")}
          title={t("search.clearRecent")}
          onmousedown={(event) => event.preventDefault()}
          onclick={clearHistory}
        ><Icon name="x" size={14} /></button>
      </div>
      <!-- The listbox wraps only the options; focus stays in the combobox. -->
      <div id="search-suggestions" role="listbox" aria-label={t("search.recent")}>
        {#each suggestions as suggestion, i (suggestion)}
          <button
            id={`search-suggestion-${i}`}
            type="button"
            role="option"
            tabindex="-1"
            aria-selected={i === active}
            class:active={i === active}
            onmouseenter={() => (active = i)}
            onmouseleave={() => (active = -1)}
            onmousedown={(event) => event.preventDefault()}
            onclick={() => submit(suggestion)}
          >
            <span class="suggest-icon"><Icon name="search" size={14} /></span>
            <span class="suggest-text">{suggestion}</span>
          </button>
        {/each}
      </div>
    </div>
  {/if}
</div>

<style>
  .search-bar {
    position: relative;
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .search-bar-input {
    flex: 1;
    min-width: 0;
    height: 48px;
    padding-left: 44px;
    font-size: 15px;
    box-shadow: var(--shadow-panel);
  }
  .search-icon {
    position: absolute;
    left: 16px;
    display: grid;
    place-items: center;
    color: var(--text-muted);
    pointer-events: none;
  }
  /* Home's launcher: a step smaller, so the greeting stays the page's lead. */
  .compact .search-bar-input {
    height: 40px;
    padding-left: 38px;
    font-size: 14px;
  }
  .compact .search-icon {
    left: 13px;
  }
  /* Recent-search dropdown: the floating-overlay recipe (darker float veil +
     slight accent tint) shared with ContextMenu, so both stay legible over
     bright art and read as one overlay family. */
  .suggest {
    --suggest-fill: color-mix(in srgb, var(--accent) 8%, var(--surface-float));
    position: absolute;
    top: calc(100% + 6px);
    left: 0;
    z-index: 10;
    min-width: 260px;
    max-width: 420px;
    padding: 4px;
    border: 1px solid transparent;
    border-radius: var(--radius-control);
    background:
      linear-gradient(var(--suggest-fill), var(--suggest-fill)) padding-box,
      var(--edge-lit) border-box;
    backdrop-filter: blur(var(--glass-blur));
    box-shadow: 0 6px 18px rgb(0 0 0 / 0.4);
  }
  .suggest-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    padding: 4px 4px 4px 10px;
    font-size: 12px;
    color: var(--text-muted);
  }
  .suggest-clear {
    display: grid;
    place-items: center;
    width: 22px;
    height: 22px;
    border-radius: 50%;
    color: var(--text-muted);
  }
  .suggest-clear:hover {
    color: var(--text);
  }
  .suggest [role="option"] {
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
    padding: 6px 10px;
    border-radius: 4px;
    text-align: left;
  }
  .suggest-icon {
    display: grid;
    place-items: center;
    flex: 0 0 auto;
    color: var(--text-muted);
  }
  .suggest [role="option"]:hover,
  .suggest [role="option"].active {
    background: var(--surface-2);
  }
  .suggest-text {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
</style>
