<script lang="ts">
  import { api } from "../lib/api";
  let { onLogin }: { onLogin: () => void } = $props();
  let url = $state("");
  let redirect = $state("");
  let error = $state("");

  async function begin() {
    const prompt = await api.authBegin();
    url = prompt.url;
    window.open(url, "_blank");
  }
  async function complete() {
    error = "";
    try { await api.authComplete(redirect); onLogin(); }
    catch (e) { error = String(e); }
  }
</script>

<div style="max-width:640px;margin:10vh auto">
  <h1>Sign in to TIDAL</h1>
  <button onclick={begin}>Open login page</button>
  {#if url}<p class="muted">After logging in you'll land on an "Oops" page. Paste its full URL below.</p>{/if}
  <input placeholder="Paste the redirect URL" bind:value={redirect} style="width:100%" />
  <button onclick={complete} disabled={!redirect}>Complete login</button>
  {#if error}<p style="color:#ff6b6b">{error}</p>{/if}
</div>
