<script lang="ts">
  import type { Nav } from "../nav.svelte";
  import { trackInfo } from "../trackInfo.svelte";
  import InfoPanel from "./InfoPanel.svelte";
  import Modal from "./Modal.svelte";

  let { nav }: { nav: Nav } = $props();

  const request = $derived(trackInfo.request);
</script>

{#if request}
  <Modal title="Track info" onclose={() => trackInfo.close()}>
    <InfoPanel trackId={request.id} {nav} onNavigate={() => trackInfo.close()} />
  </Modal>
{/if}
