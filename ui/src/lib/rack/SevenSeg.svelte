<script lang="ts">
  import { segGhost, segLit } from "./segment";

  /// 7-segment readout for digits and colons (DESIGN §4), set in the vendored
  /// DSEG7 face. Unlit segments stay faintly visible, like a real VFD.
  /// Decorative: the caller exposes the value as text elsewhere.
  let { value, height = 22 }: { value: string; height?: number } = $props();
</script>

<span class="seg" style:font-size="{height}px" aria-hidden="true">
  <span class="ghost">{segGhost(value)}</span><span class="lit">{segLit(value)}</span>
</span>

<style>
  .seg {
    position: relative;
    display: block;
    font: italic 700 1em/1 "DSEG7 Classic", monospace;
    white-space: pre;
    font-kerning: none;
  }
  .ghost {
    color: var(--rack-vfd-ghost);
  }
  .lit {
    position: absolute;
    inset: 0;
    color: var(--rack-vfd);
    text-shadow: 0 0 6px color-mix(in srgb, var(--rack-vfd) 70%, transparent), 0 0 1px var(--rack-vfd);
  }
</style>
