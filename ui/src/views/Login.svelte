<script lang="ts">
  import { api } from "../lib/api";
  import { openUrl } from "@tauri-apps/plugin-opener";
  let { onLogin, message = "" }: { onLogin: () => void; message?: string } = $props();
  let url = $state("");
  let redirect = $state("");
  let error = $state("");
  let busy = $state(false);

  async function begin() {
    error = "";
    busy = true;
    try {
      const prompt = await api.authBegin();
      url = prompt.url;
      await openUrl(url);
    } catch (e) {
      error = String(e);
    } finally {
      busy = false;
    }
  }

  async function complete() {
    error = "";
    busy = true;
    try {
      await api.authComplete(redirect);
      onLogin();
    } catch (e) {
      error = String(e);
    } finally {
      busy = false;
    }
  }
</script>

<div style="max-width:640px;margin:10vh auto">
  <h1>{message || "Sign in to TIDAL"}</h1>
  <button onclick={begin} disabled={busy}>Open login page</button>
  {#if url}<p class="muted">After logging in you'll land on an "Oops" page. Paste its full URL below.</p>{/if}
  <input placeholder="Paste the redirect URL" bind:value={redirect} style="width:100%" />
  <button onclick={complete} disabled={busy || !redirect}>Complete login</button>
  {#if error}<p style="color:var(--danger)">{error}</p>{/if}
</div>
