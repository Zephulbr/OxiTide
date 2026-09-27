<script lang="ts">
  import { api } from "../lib/api";
  import { openUrl } from "@tauri-apps/plugin-opener";
  let {
    onLogin,
    message = "",
    authError = "",
  }: { onLogin: () => void; message?: string; authError?: string } = $props();
  let url = $state("");
  let redirect = $state("");
  let error = $state("");
  let busy = $state(false);
  let waiting = $state(false);
  let showFallback = $state(false);

  async function begin(mode: "deep" | "paste") {
    error = "";
    busy = true;
    try {
      const prompt = await api.authBegin(mode);
      url = prompt.url;
      waiting = mode === "deep";
      await openUrl(url);
    } catch (e) {
      error = String(e);
      waiting = false;
    } finally {
      busy = false;
    }
  }

  function reset() {
    waiting = false;
    error = "";
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
  <button onclick={() => begin("deep")} disabled={busy}>Log in with TIDAL</button>
  {#if waiting}
    <p class="muted">Finish signing in in your browser. OxiTide will unlock automatically.</p>
    <button onclick={reset} disabled={busy}>Cancel</button>
  {/if}
  <p>
    <button class="link" onclick={() => (showFallback = !showFallback)}>
      Browser didn't open the app?
    </button>
  </p>
  {#if showFallback}
    <button onclick={() => begin("paste")} disabled={busy}>Open browser (paste-back)</button>
    {#if url && !waiting}
      <p class="muted">After logging in you'll land on an "Oops" page. Paste its full URL below.</p>
    {/if}
    <input placeholder="Paste the redirect URL" bind:value={redirect} style="width:100%" />
    <button onclick={complete} disabled={busy || !redirect}>Complete login</button>
  {/if}
  {#if error || authError}<p style="color:var(--danger)">{error || authError}</p>{/if}
</div>
