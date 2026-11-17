<script lang="ts">
  import { api, type CrossfadeSettings, type TransitionReplay, type TransitionStyle } from "../../api";
  import {
    TRANSITION_STYLES,
    describeTransition,
    formatReplayTime,
    nearEndTarget,
  } from "../../dsp";
  import { errorMessage } from "../../errors";
  import type { Player } from "../../player.svelte";
  import Button from "./Button.svelte";
  import Notice from "./Notice.svelte";
  import SettingRow from "./SettingRow.svelte";
  import Segmented from "./Segmented.svelte";
  import ToggleRow from "./ToggleRow.svelte";
  import { t } from "../../i18n.svelte";

  /// AutoMix (experimental): transition style, the individually toggled
  /// helpers, the "last transition" readout and the replay lab. The replay
  /// lab's audio state lives here, so leaving the page stops it.
  let {
    crossfade,
    player,
    onchange,
    disabled = false,
  }: {
    crossfade: CrossfadeSettings;
    player: Player;
    onchange: (patch: Partial<CrossfadeSettings>) => void;
    /// Shown but inert while there is no crossfade to build on.
    disabled?: boolean;
  } = $props();

  let replay = $state<{ style: TransitionStyle; result: TransitionReplay } | null>(null);
  let replayPlaying = $state<TransitionStyle | null>(null);
  let replayBusy = $state(false);
  let replayError = $state("");
  let replayAudio: HTMLAudioElement | null = null;
  let replayUrl = "";

  const styleHint = $derived(TRANSITION_STYLES.find((s) => s.id === crossfade.style)?.hint ?? "");

  function stopReplay() {
    replayAudio?.pause();
    replayAudio = null;
    replayPlaying = null;
    if (replayUrl) URL.revokeObjectURL(replayUrl);
    replayUrl = "";
  }

  async function playReplay(style: TransitionStyle) {
    replayBusy = true;
    replayError = "";
    try {
      const result = await api.automixRender(style);
      stopReplay();
      if (player.state.playing) await player.playPause();
      replay = { style, result };
      replayUrl = URL.createObjectURL(result.wav);
      const audio = new Audio(replayUrl);
      audio.onended = () => {
        if (replayAudio === audio) replayPlaying = null;
      };
      replayAudio = audio;
      replayPlaying = style;
      await audio.play();
    } catch (e) {
      replayError = errorMessage(e);
      stopReplay();
    } finally {
      replayBusy = false;
    }
  }

  $effect(() => stopReplay);
</script>

<SettingRow
  id="automix-style" label={t("settings.automixStyle.label")}
  description={styleHint}
  {disabled}
  stacked
>
  <Segmented
    label={t("settings.automixStyle.label")}
    {disabled}
    value={crossfade.style}
    options={TRANSITION_STYLES.map((s) => ({ value: s.id, label: s.label, hint: s.hint }))}
    onchange={(style) => onchange({ style })}
  />
</SettingRow>
<ToggleRow
  id="automix-trim" label={t("settings.automixTrim.label")}
  description={t("settings.automixTrim.description")}
  {disabled}
  checked={crossfade.trim_silence}
  onchange={(trim_silence) => onchange({ trim_silence })}
/>
<ToggleRow
  id="automix-resample" label={t("settings.automixResample.label")}
  description={t("settings.automixResample.description")}
  {disabled}
  checked={crossfade.resample}
  onchange={(resample) => onchange({ resample })}
/>
<ToggleRow
  id="automix-beat" label={t("settings.automixBeat.label")}
  description={t("settings.automixBeat.description")}
  {disabled}
  checked={crossfade.beat_match}
  onchange={(beat_match) => onchange({ beat_match })}
/>

<SettingRow
  label={t("settings.tryIt.label")}
  description={t("settings.tryIt.description")}
  {disabled}
>
  {#if player.state.current && player.state.duration_ms > 0}
    <Button
      icon="skip-forward"
      {disabled}
      onclick={() => player.seek(nearEndTarget(player.state.duration_ms))}
    >
      {t("automix.jumpNearEnd")}
    </Button>
  {:else}
    <span class="muted small">{t("automix.playToTry")}</span>
  {/if}
</SettingRow>

{#if player.state.last_transition}
  {@const summary = describeTransition(player.state.last_transition)}
  <SettingRow label={t("settings.lastTransition.label")} stacked>
    <div class="report" aria-live="polite">
      <strong>{summary.title}</strong>
      <ul>
        {#each summary.details as line (line)}<li>{line}</li>{/each}
      </ul>
    </div>
  </SettingRow>
{/if}

<SettingRow
  id="automix-replay" label={t("settings.automixReplay.label")}
  description={t("settings.automixReplay.description")}
  {disabled}
  stacked
>
  <div class="replay" role="group" aria-label={t("automix.replayGroup")}>
    {#each TRANSITION_STYLES as option (option.id)}
      <Button
        icon={replayPlaying === option.id ? "square" : "play"}
        disabled={replayBusy || disabled}
        aria-pressed={replayPlaying === option.id}
        onclick={() => (replayPlaying === option.id ? stopReplay() : playReplay(option.id))}
      >{replayPlaying === option.id ? t("automix.stopReplay", { style: option.label }) : option.label}</Button>
    {/each}
  </div>
</SettingRow>
{#if replayError}
  <div class="status"><Notice kind="warning">{replayError}</Notice></div>
{:else if replay}
  {@const summary = describeTransition(replay.result.report)}
  <p class="status muted small" aria-live="polite">
    {summary.title}: overlap {formatReplayTime(replay.result.overlap_start_ms)}–{formatReplayTime(replay.result.overlap_end_ms)}
    · {summary.details.join(" · ")}
  </p>
{/if}

<style>
  .small {
    font-size: 12.5px;
  }
  .report {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .report ul {
    margin: 0;
    padding-left: 18px;
    color: var(--text-muted);
    font-size: 12.5px;
    line-height: 1.55;
  }
  .replay {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
  }
  .status {
    margin: 0;
    padding: 0 18px 14px;
  }
  .status :global(.notice) {
    margin: 0;
  }
</style>
