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
  import Button from "./settings/Button.svelte";
  import Notice from "./settings/Notice.svelte";
  import SettingRow from "./settings/SettingRow.svelte";
  import SettingsSection from "./settings/SettingsSection.svelte";
  import ToggleRow from "./settings/ToggleRow.svelte";

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

  /// The status line under the version; installing has its own live region.
  const message = $derived.by(() => {
    if (!support) return "";
    if (!support.supported) return support.reason ?? "";
    if (phase === "checking") return "Checking for updates…";
    if (phase === "current") return "You are on the latest version.";
    if (phase === "available" && update) return `Version ${update.version} is available.`;
    return "";
  });
</script>

<SettingsSection title="Updates" icon="download">
  {#if support}
    <SettingRow label={`OxiTide ${support.current_version}`} description={message}>
      {#if support.supported}
        {#if phase === "available"}
          <Button variant="primary" icon="download" onclick={install}>Download and install</Button>
        {:else}
          <Button
            icon="refresh-cw"
            disabled={phase === "checking" || phase === "installing"}
            onclick={check}
          >Check for updates</Button>
        {/if}
      {/if}
      {#snippet below()}
        {#if phase === "available" && update?.notes}
          <span class="notes">{update.notes}</span>
        {/if}
        {#if phase === "installing"}
          <span class="installing" role="status">
            Downloading{percent === null ? "…" : ` ${percent}%`} — OxiTide restarts when it finishes.
          </span>
          <progress max="100" value={percent ?? undefined} aria-label="Update download"></progress>
        {/if}
      {/snippet}
    </SettingRow>
    {#if support.supported}
      <ToggleRow
        label="Check for updates when OxiTide starts"
        checked={support.check_on_start}
        onchange={setCheckOnStart}
      />
    {/if}
  {/if}
  {#if error}
    <div class="inset"><Notice kind="warning">{error}</Notice></div>
  {/if}
</SettingsSection>

<style>
  .notes {
    display: block;
    max-height: 8em;
    overflow: auto;
    color: var(--text-muted);
    font-size: 12.5px;
    white-space: pre-wrap;
  }
  .installing {
    display: block;
    margin-bottom: 8px;
    color: var(--text-muted);
    font-size: 12.5px;
  }
  progress {
    width: 100%;
    height: 8px;
    accent-color: var(--accent);
  }
  .inset {
    padding: 14px 18px 0;
  }
</style>
