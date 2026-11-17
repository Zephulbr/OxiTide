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
  import ConfirmDialog from "./ConfirmDialog.svelte";
  import Button from "./settings/Button.svelte";
  import Notice from "./settings/Notice.svelte";
  import SettingRow from "./settings/SettingRow.svelte";
  import SettingsSection from "./settings/SettingsSection.svelte";
  import ToggleRow from "./settings/ToggleRow.svelte";
  import { t } from "../i18n.svelte";

  type Phase = "idle" | "checking" | "current" | "available" | "installing";

  let support = $state<UpdateSupport | null>(null);
  let phase = $state<Phase>("idle");
  let update = $state<UpdateInfo | null>(null);
  let progress = $state<UpdateProgress | null>(null);
  let error = $state("");
  /// Installing runs code fetched from GitHub, so it is never one click.
  let confirming = $state(false);

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
    confirming = false;
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
    if (phase === "checking") return t("updates.checking");
    if (phase === "current") return t("updates.current");
    if (phase === "available" && update) return t("updates.available", { version: update.version });
    return "";
  });
</script>

<SettingsSection title={t("settings.section.updates")} icon="download">
  {#if support}
    <SettingRow id="updates-check" label={`OxiTide ${support.current_version}`} description={message}>
      {#if support.supported}
        {#if phase === "available"}
          <Button variant="primary" icon="download" onclick={() => (confirming = true)}>{t("updates.install")}</Button>
        {:else}
          <Button
            icon="refresh-cw"
            disabled={phase === "checking" || phase === "installing"}
            onclick={check}
          >{t("settings.updatesCheck.label")}</Button>
        {/if}
      {/if}
      {#snippet below()}
        {#if phase === "available" && update?.notes}
          <span class="notes">{update.notes}</span>
        {/if}
        {#if phase === "installing"}
          <span class="installing" role="status">
            {percent === null ? t("updates.downloading") : t("updates.downloadingPercent", { percent })}
          </span>
          <progress max="100" value={percent ?? undefined} aria-label={t("updates.download")}></progress>
        {/if}
      {/snippet}
    </SettingRow>
    {#if support.supported}
      <ToggleRow
        id="updates-start" label={t("settings.updatesStart.label")}
        checked={support.check_on_start}
        onchange={setCheckOnStart}
      />
    {/if}
  {/if}
  {#if error}
    <div class="inset"><Notice kind="warning">{error}</Notice></div>
  {/if}
</SettingsSection>
{#if confirming && update}
  <ConfirmDialog
    title={t("updates.confirmTitle", { version: update.version })}
    message={t("updates.confirmMessage")}
    confirmLabel={t("updates.install")}
    destructive={false}
    onconfirm={install}
    oncancel={() => (confirming = false)}
  />
{/if}

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
