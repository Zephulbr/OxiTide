<script lang="ts">
  import { openUrl } from "@tauri-apps/plugin-opener";
  import { pickerLanguages, t } from "../../i18n.svelte";
  import { language, systemLanguage } from "../../language.svelte";
  import { LANGUAGES, PSEUDO_LANGUAGE, SYSTEM_LANGUAGE, type LanguageStatus } from "../../languages";
  import Icon from "../Icon.svelte";
  import Select from "./Select.svelte";
  import SettingRow from "./SettingRow.svelte";

  /// Settings → Appearance → Language (spec §5.3): "System (<endonym>)", then
  /// every language by its own name, tagged with its translation status.
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

  const options = $derived([
    { value: SYSTEM_LANGUAGE, label: t("settings.language.system", { language: endonym(systemLanguage()) }) },
    ...pickerLanguages().map((code) => ({
      value: code,
      label: endonym(code),
      hint: statusHint(LANGUAGES.find((l) => l.code === code)?.status),
    })),
  ]);

  function choose(value: string) {
    language.set(value).catch(onerror);
  }
</script>

<SettingRow id="language" label={t("settings.language.label")} description={t("settings.language.description")}>
  <Select label={t("settings.language.label")} value={language.setting} {options} onchange={choose} />
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
