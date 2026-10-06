<script lang="ts">
  import { lyrics } from "../lyrics.svelte";
  import type { Nav } from "../nav.svelte";
  import type { Player } from "../player.svelte";
  import { trackInfo } from "../trackInfo.svelte";
  import InfoPanel from "./InfoPanel.svelte";
  import Modal from "./Modal.svelte";

  let { player, nav }: { player: Player; nav: Nav } = $props();

  const request = $derived(trackInfo.request);
  // The playing track opens Now Playing's Info tab; any other row opens the
  // modal with the same panel (spec §6.2).
  const isCurrent = $derived(
    request !== null && player.state.current?.track_id === request.id,
  );

  $effect(() => {
    if (request && isCurrent) {
      lyrics.setTab("info");
      if (!nav.nowPlayingOpen) nav.toggleNowPlaying();
      trackInfo.close();
    }
  });
</script>

{#if request && !isCurrent}
  <Modal title="Track info" onclose={() => trackInfo.close()}>
    <InfoPanel trackId={request.id} {nav} onNavigate={() => trackInfo.close()} />
  </Modal>
{/if}
