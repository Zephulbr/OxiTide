<script lang="ts">
  import { api } from "../lib/api";
  import { openUrl } from "@tauri-apps/plugin-opener";
  import Loading from "../lib/components/Loading.svelte";
  import KeyringNotice from "../lib/components/KeyringNotice.svelte";
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

  // A deep-link failure arrives while we are still "waiting": drop back to the
  // idle screen so the message and the retry button are both reachable.
  $effect(() => {
    if (authError) waiting = false;
  });

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

<div class="login">
  <div class="card lit">
    <h1>{message || "Sign in to TIDAL"}</h1>
    <p class="intro muted">
      OxiTide opens TIDAL in your browser. Sign in — the app unlocks automatically when you return.
    </p>
    <KeyringNotice />

    {#if waiting}
      <div class="waiting">
        <Loading label="Waiting for your browser…" />
        <button class="secondary" onclick={reset} disabled={busy}>Cancel</button>
      </div>
    {:else}
      <button class="primary" onclick={() => begin("deep")} disabled={busy}>
        Log in with TIDAL
      </button>
    {/if}

    <button
      class="link"
      onclick={() => (showFallback = !showFallback)}
      aria-expanded={showFallback}
    >
      Browser didn't open the app?
    </button>

    {#if showFallback}
      <div class="fallback">
        <button class="secondary" onclick={() => begin("paste")} disabled={busy}>
          Open browser (paste-back)
        </button>
        {#if url && !waiting}
          <p class="muted">
            After logging in you'll land on an "Oops" page. Paste its full URL below.
          </p>
        {/if}
        <input placeholder="Paste the redirect URL" bind:value={redirect} />
        <button class="secondary" onclick={complete} disabled={busy || !redirect}>
          Complete login
        </button>
      </div>
    {/if}

    {#if error || authError}<p class="error">{error || authError}</p>{/if}
  </div>
</div>

<style>
  .login {
    min-height: 100%;
    display: grid;
    place-items: center;
    padding: 24px;
  }
  .card {
    width: min(440px, 100%);
    display: flex;
    flex-direction: column;
    gap: 12px;
    padding: 28px;
    border-radius: var(--radius-card);
    box-shadow: 0 18px 48px rgba(0, 0, 0, 0.35);
  }
  .card h1 {
    margin: 0;
    font-size: 26px;
  }
  .intro {
    margin: 0;
    line-height: 1.5;
  }
  .waiting {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
  }
  .primary {
    padding: 12px 18px;
    border-radius: var(--radius-control);
    background: var(--accent);
    color: var(--on-accent);
    font-weight: 600;
    font-size: 15px;
  }
  .primary:not(:disabled):hover {
    filter: brightness(1.06);
  }
  .secondary {
    padding: 10px 14px;
    border-radius: var(--radius-control);
    background: var(--surface-2);
    border: 1px solid var(--line);
    color: var(--text);
  }
  .secondary:not(:disabled):hover {
    background: var(--surface-3);
  }
  .link {
    align-self: center;
    padding: 4px;
    color: var(--text-muted);
    text-decoration: underline;
    text-underline-offset: 3px;
  }
  .link:hover {
    color: var(--text);
  }
  .fallback {
    display: flex;
    flex-direction: column;
    gap: 10px;
    padding-top: 12px;
    border-top: 1px solid var(--line);
  }
  .fallback p {
    margin: 0;
    line-height: 1.5;
  }
  input {
    width: 100%;
    padding: 10px 12px;
    border-radius: var(--radius-control);
    background: var(--surface-2);
    border: 1px solid var(--line);
    color: var(--text);
  }
  .error {
    margin: 0;
    color: var(--danger);
  }
</style>
