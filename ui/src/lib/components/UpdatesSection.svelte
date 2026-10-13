<script lang="ts">
  import { listen } from "@tauri-apps/api/event";
  import {
    api,
    UPDATE_PROGRESS_EVENT,
    type UpdateInfo,
    type UpdateProgress,
    type UpdateSupport,
  } from "../api";
  import { errorMessage } from "../errors";

  type Phase = "idle" | "checking" | "current" | "available" | "installing";

  let support = $state<UpdateSupport | null>(null);
  let phase = $state<Phase>("idle");
  let update = $state<UpdateInfo | null>(null);
  let progress = $state<UpdateProgress | null>(null);
  let error = $state("");

  $effect(() => {
    api
      .getUpdateSupport()
      .then((value) => (support = value))
      .catch((e) => (error = errorMessage(e)));
    const unlisten = listen<UpdateProgress>(UPDATE_PROGRESS_EVENT, (event) => {
      progress = event.payload;
    });
    return () => {
      void unlisten.then((off) => off());
    };
  });

  async function check() {
    phase = "checking";
    error = "";
    try {
      update = await api.checkForUpdate();
      phase = update ? "available" : "current";
    } catch (e) {
      error = errorMessage(e);
      phase = "idle";
    }
  }

  async function install() {
    phase = "installing";
    progress = null;
    error = "";
    try {
      await api.installUpdate();
    } catch (e) {
      error = errorMessage(e);
      phase = "available";
    }
  }

  async function setCheckOnStart(enabled: boolean) {
    if (!support) return;
    const previous = support.check_on_start;
    support = { ...support, check_on_start: enabled };
    try {
      await api.setUpdateCheckOnStart(enabled);
    } catch (e) {
      support = { ...support, check_on_start: previous };
      error = errorMessage(e);
    }
  }

  const percent = $derived(
    progress?.total ? Math.min(100, Math.round((progress.downloaded / progress.total) * 100)) : null,
  );
</script>

<h3>Updates</h3>
{#if support}
  <div class="row">
    <div class="main">
      <span>OxiTide {support.current_version}</span>
      {#if !support.supported}
        <span class="muted">{support.reason}</span>
      {:else if phase === "checking"}
        <span class="muted">Checking for updates…</span>
      {:else if phase === "current"}
        <span class="muted">You are on the latest version.</span>
      {:else if phase === "available" && update}
        <span>Version {update.version} is available.</span>
        {#if update.notes}<span class="muted notes">{update.notes}</span>{/if}
      {:else if phase === "installing"}
        <span class="muted" role="status">
          Downloading{percent === null ? "…" : ` ${percent}%`} — OxiTide restarts when it finishes.
        </span>
      {/if}
    </div>
    {#if support.supported}
      {#if phase === "available"}
        <button class="primary" onclick={install}>Download and install</button>
      {:else}
        <button
          class="secondary"
          disabled={phase === "checking" || phase === "installing"}
          onclick={check}
        >Check for updates</button>
      {/if}
    {/if}
  </div>
  {#if phase === "installing"}
    <progress max="100" value={percent ?? undefined} aria-label="Update download"></progress>
  {/if}
  {#if support.supported}
    <label class="head">
      <input
        type="checkbox"
        checked={support.check_on_start}
        onchange={(e) => setCheckOnStart((e.target as HTMLInputElement).checked)}
      />
      <span>Check for updates when OxiTide starts</span>
    </label>
  {/if}
{/if}
{#if error}
  <p class="warn" role="alert">{error}</p>
{/if}

<style>
  .row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    margin: 8px 0;
    padding: 10px 12px;
    border: 1px solid var(--line);
    border-radius: var(--radius-control);
    background: var(--surface-1);
  }
  .main {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
  }
  .notes {
    white-space: pre-wrap;
    max-height: 8em;
    overflow: auto;
  }
  .head {
    display: flex;
    align-items: center;
    gap: 8px;
    margin: 8px 0;
  }
  progress {
    width: 100%;
    accent-color: var(--accent);
  }
  .warn {
    color: var(--warning);
  }
  button.primary,
  button.secondary {
    padding: 6px 14px;
    border-radius: var(--radius-control);
    font-weight: 600;
    flex: none;
  }
  button.primary {
    background: var(--accent);
    color: var(--on-accent);
  }
  button.secondary {
    border: 1px solid var(--line);
    background: var(--surface-2);
    color: var(--text-muted);
  }
</style>
