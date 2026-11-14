<script lang="ts">
  import { openUrl } from "@tauri-apps/plugin-opener";
  import { tick } from "svelte";
  import { appearance } from "../../appearance.svelte";
  import { i18n, pickerLanguages, t, tEn } from "../../i18n.svelte";
  import { language, systemLanguage } from "../../language.svelte";
  import { LANGUAGES, PSEUDO_LANGUAGE, SYSTEM_LANGUAGE, type LanguageStatus } from "../../languages";
  import { retype, snapshotText } from "../../retype";
  import Icon from "../Icon.svelte";
  import Select from "./Select.svelte";
  import SettingRow from "./SettingRow.svelte";

  /// Settings → Appearance → Language (spec §5.3): "System (<endonym>)", then
  /// every language by its own name, tagged with its translation status. The
  /// list filters by the language's own name, its name in the current
  /// language and in English, or its code ("German", "Allemand", "de").
  let { onerror }: { onerror: (failure: unknown) => void } = $props();

  /// The translating guide; volunteers fix or add languages there (spec §8).
  const GUIDE_URL = "https://github.com/Zephulbr/OxiTide/blob/main/CONTRIBUTING.md#translating";

  function endonym(code: string): string {
    if (code === PSEUDO_LANGUAGE) return t("settings.language.pseudo");
    return LANGUAGES.find((l) => l.code === code)?.endonym ?? code;
  }

  function statusHint(status: LanguageStatus | undefined): string | undefined {
    if (status === "machine") return t("settings.language.machine");
    if (status === "cross-checked") return t("settings.language.crossChecked");
    return undefined;
  }

  /// What `code` is called in `locale`, or nothing where `Intl` cannot say.
  function nameIn(locale: string, code: string): string | undefined {
    try {
      return new Intl.DisplayNames([locale], { type: "language" }).of(code);
    } catch {
      return undefined;
    }
  }

  function keywords(code: string): string[] {
    if (code === PSEUDO_LANGUAGE) return [code];
    return [nameIn(i18n.locale, code), nameIn("en", code), code].filter((k): k is string => !!k);
  }

  const options = $derived([
    {
      value: SYSTEM_LANGUAGE,
      label: t("settings.language.system", { language: endonym(systemLanguage()) }),
      keywords: [tEn("settings.language.system", { language: endonym(systemLanguage()) })],
    },
    ...pickerLanguages().map((code) => ({
      value: code,
      label: endonym(code),
      hint: statusHint(LANGUAGES.find((l) => l.code === code)?.status),
      keywords: keywords(code),
    })),
  ]);

  /// Switches, then erases the visible text and types it again in the new
  /// language (skipped under reduced motion). The snapshot is taken before
  /// the switch; the animation starts in the same task as the re-render, so
  /// the new text never paints before its erase.
  function choose(value: string) {
    const before = appearance.reducedMotion ? null : snapshotText();
    const onApplied = before ? () => void tick().then(() => retype(before)) : undefined;
    language.set(value, onApplied).catch(onerror);
  }
</script>

<SettingRow id="language" label={t("settings.language.label")} description={t("settings.language.description")}>
  <Select
    label={t("settings.language.label")}
    value={language.setting}
    {options}
    onchange={choose}
    search={{ label: t("settings.language.search"), empty: t("settings.language.noMatch") }}
  />
  {#snippet below()}
    <button class="link" type="button" onclick={() => openUrl(GUIDE_URL).catch(onerror)}>
      <Icon name="external-link" size={13} />{t("settings.language.helpTranslate")}
    </button>
  {/snippet}
</SettingRow>

<style>
  .link {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 0;
    border: 0;
    background: none;
    color: var(--accent);
    font-size: 12.5px;
    cursor: pointer;
  }
  .link:hover {
    text-decoration: underline;
  }
</style>
