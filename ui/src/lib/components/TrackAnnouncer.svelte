<script lang="ts">
  import { announcer as shared, type Announcer } from "../announce.svelte";

  /// The polite live region for user-started track changes (A-4). Mounted
  /// once by App, in every shell (default, Rack, mini player).
  let { announcer = shared }: { announcer?: Announcer } = $props();
</script>

<div class="sr-only" aria-live="polite" aria-atomic="true">{announcer.message}</div>

<style>
  .sr-only {
    position: absolute;
    width: 1px;
    height: 1px;
    overflow: hidden;
    clip-path: inset(50%);
    white-space: nowrap;
  }
</style>
