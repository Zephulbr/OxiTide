<script lang="ts">
  import type { Snippet } from "svelte";
  import SettingRow from "./SettingRow.svelte";
  import Toggle from "./Toggle.svelte";

  /// A `SettingRow` whose control is a switch; the whole row toggles it.
  let {
    label,
    description,
    checked,
    disabled = false,
    onchange,
    below,
  }: {
    label: string;
    description?: string;
    checked: boolean;
    disabled?: boolean;
    onchange: (checked: boolean) => void;
    below?: Snippet;
  } = $props();
</script>

<SettingRow {label} {description} {disabled} tag="label" {below}>
  <Toggle {checked} {label} {disabled} {onchange} />
</SettingRow>
