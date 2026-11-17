<script lang="ts">
  import type { Nav } from "../nav.svelte";
  import { trackInfo } from "../trackInfo.svelte";
  import InfoPanel from "./InfoPanel.svelte";
  import Modal from "./Modal.svelte";
  import { t } from "../i18n.svelte";

  let { nav }: { nav: Nav } = $props();

  const request = $derived(trackInfo.request);
</script>

{#if request}
  <Modal title={t("menu.trackInfo")} onclose={() => trackInfo.close()}>
    <InfoPanel trackId={request.id} {nav} onNavigate={() => trackInfo.close()} />
  </Modal>
{/if}
