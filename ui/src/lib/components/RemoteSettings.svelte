<script lang="ts">
  import { listen } from "@tauri-apps/api/event";
  import {
    api,
    REMOTE_STATUS_EVENT,
    type RemoteDevice,
    type RemotePairing,
    type RemoteStatus,
  } from "../api";
  import { errorMessage } from "../errors";
  import { countdownLabel, firewallHint, lastSeenLabel, parsePort } from "../remoteSettings";
  import Icon from "./Icon.svelte";
  import Modal from "./Modal.svelte";

  let status = $state<RemoteStatus | null>(null);
  let devices = $state<RemoteDevice[]>([]);
  let pairing = $state<RemotePairing | null>(null);
  let remaining = $state(0);
  let busy = $state(false);
  let error = $state("");
  let portText = $state("");
  let now = $state(Date.now());

  async function refresh() {
    try {
      const [next, list] = await Promise.all([api.getRemoteStatus(), api.listRemoteDevices()]);
      apply(next);
      devices = list;
    } catch (e) {
      error = errorMessage(e);
    }
  }

  function apply(next: RemoteStatus) {
    status = next;
    portText = String(next.settings.port);
    // Expired, used by a phone, or the server stopped: the QR is dead.
    const spent = next.pairing_expires_in_secs === null && next.pending.length === 0;
    if (pairing && (!next.listening || spent)) pairing = null;
  }

  $effect(() => {
    void refresh();
    const unlisten = listen<RemoteStatus>(REMOTE_STATUS_EVENT, (event) => {
      apply(event.payload);
      void api.listRemoteDevices().then((list) => (devices = list)).catch(() => {});
    });
    const clock = setInterval(() => (now = Date.now()), 30_000);
    return () => {
      clearInterval(clock);
      void unlisten.then((off) => off());
    };
  });

  $effect(() => {
    if (!pairing) return;
    const started = Date.now();
    const total = pairing.expires_in_secs;
    remaining = total;
    const tick = setInterval(() => {
      remaining = Math.max(0, total - (Date.now() - started) / 1000);
      if (remaining <= 0) pairing = null;
    }, 1000);
    return () => clearInterval(tick);
  });

  async function setEnabled(enabled: boolean) {
    busy = true;
    error = "";
    try {
      apply(await api.setRemoteEnabled(enabled));
      if (!enabled) pairing = null;
    } catch (e) {
      error = errorMessage(e);
    } finally {
      busy = false;
    }
  }

  async function saveSettings(patch: { port?: number; iface?: string; approve?: boolean }) {
    if (!status) return;
    const s = status.settings;
    error = "";
    try {
      apply(
        await api.setRemoteSettings(
          patch.port ?? s.port,
          patch.iface ?? s.interface,
          patch.approve ?? s.approve_new_devices,
        ),
      );
    } catch (e) {
      error = errorMessage(e);
    }
  }

  function commitPort() {
    const port = parsePort(portText);
    if (port === null) {
      portText = String(status?.settings.port ?? "");
      error = "Use a port between 1024 and 65526.";
      return;
    }
    if (port !== status?.settings.port) void saveSettings({ port });
  }

  async function beginPairing() {
    error = "";
    try {
      pairing = await api.beginRemotePairing();
    } catch (e) {
      error = errorMessage(e);
    }
  }

  async function closePairing() {
    pairing = null;
    await api.cancelRemotePairing().catch(() => {});
  }

  async function resolve(id: string, approve: boolean) {
    try {
      await api.resolveRemotePairRequest(id, approve);
      if (approve) pairing = null;
    } catch (e) {
      error = errorMessage(e);
    }
  }

  async function revoke(device: RemoteDevice) {
    try {
      await api.revokeRemoteDevice(device.id);
      devices = devices.filter((d) => d.id !== device.id);
    } catch (e) {
      error = errorMessage(e);
    }
  }

  const hint = $derived(firewallHint(navigator.userAgent, status?.port ?? status?.settings.port ?? 47820));
</script>

{#snippet approvals()}
  {#each status?.pending ?? [] as request (request.id)}
    <div class="approval" role="alert">
      <span>Pair <strong>{request.name}</strong> from {request.ip}?</span>
      <span class="actions">
        <button class="secondary" onclick={() => resolve(request.id, false)}>Decline</button>
        <button class="primary" onclick={() => resolve(request.id, true)}>Allow</button>
      </span>
    </div>
  {/each}
{/snippet}

<h3>Phone remote</h3>
<div class="row">
  <div class="main">
    <label class="head">
      <input
        type="checkbox"
        checked={status?.enabled ?? false}
        disabled={busy || !status}
        onchange={(e) => setEnabled((e.target as HTMLInputElement).checked)}
      />
      <span>Control OxiTide from your phone</span>
    </label>
    {#if status?.listening && status.url}
      <span class="muted">
        Listening on {status.url} ·
        {status.connected === 1 ? "1 phone connected" : `${status.connected} phones connected`}
      </span>
    {:else if status?.enabled && status.error}
      <span class="warn">Not listening: {status.error}</span>
    {:else}
      <span class="muted">Off. A phone on the same network opens a remote page — nothing to install.</span>
    {/if}
  </div>
  {#if status?.listening}
    <button class="primary" onclick={beginPairing}>Pair a phone</button>
  {/if}
</div>

{@render approvals()}

{#if status}
  <div class="field">
    <label class="field-label" for="remote-interface">Address in the QR code</label>
    <select
      id="remote-interface"
      value={status.settings.interface}
      onchange={(e) => saveSettings({ iface: (e.target as HTMLSelectElement).value })}
    >
      <option value="">Automatic{status.address ? ` (${status.address})` : ""}</option>
      {#each status.interfaces as iface (iface.ip)}
        <option value={iface.ip}>{iface.ip} — {iface.name}</option>
      {/each}
    </select>
  </div>
  <div class="field">
    <label class="field-label" for="remote-port">Port</label>
    <input
      id="remote-port"
      class="port"
      inputmode="numeric"
      bind:value={portText}
      onchange={commitPort}
    />
    {#if status.port && status.port !== status.settings.port}
      <span class="muted">(in use; listening on {status.port})</span>
    {/if}
  </div>
  <label class="field">
    <input
      type="checkbox"
      checked={status.settings.approve_new_devices}
      onchange={(e) => saveSettings({ approve: (e.target as HTMLInputElement).checked })}
    />
    <span>Approve new phones on this PC</span>
  </label>
{/if}

<h4>Paired phones</h4>
{#if devices.length === 0}
  <p class="muted">No phones paired yet.</p>
{:else}
  <ul class="devices">
    {#each devices as device (device.id)}
      <li>
        <span class="dot" class:on={device.connected} aria-hidden="true"></span>
        <span class="name">{device.name}</span>
        <span class="muted">
          {device.connected ? "Connected" : `Last seen ${lastSeenLabel(device.last_seen_ms, now)}`}
        </span>
        <button class="danger" onclick={() => revoke(device)} aria-label={`Revoke ${device.name}`}>
          Revoke
        </button>
      </li>
    {/each}
  </ul>
{/if}

{#if error}<p class="warn">{error}</p>{/if}
<p class="muted">{hint}</p>
<p class="muted">
  The remote works on your home network only. Artwork loads from TIDAL's servers, so the phone
  needs internet access. Keep the phone screen on while you use it.
</p>

{#if pairing}
  <Modal title="Pair a phone" onclose={closePairing}>
    <div class="pairing">
      <!-- The SVG is generated by OxiTide itself (qrcode crate), not remote input. -->
      <div class="qr" aria-label="Pairing QR code" role="img">{@html pairing.qr_svg}</div>
      <p>Scan with your phone's camera, or open:</p>
      <code class="url">{pairing.url}</code>
      <p class="muted">
        Same Wi-Fi as this PC. Expires in {countdownLabel(remaining)}.
      </p>
      {@render approvals()}
      <p class="actions">
        <button class="secondary" onclick={closePairing}>Cancel</button>
      </p>
    </div>
  </Modal>
{/if}

<style>
  .row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    margin: 8px 0;
    padding: 10px 12px;
    border: 1px solid var(--line);
    border-radius: var(--radius-control);
    background: var(--surface-1);
  }
  .main {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
  }
  .head {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .field {
    display: flex;
    align-items: center;
    gap: 8px;
    margin: 8px 0;
  }
  .field-label {
    color: var(--text-muted);
  }
  .port {
    width: 7ch;
  }
  .warn {
    color: var(--warning);
  }
  h4 {
    margin: 16px 0 4px;
    font-size: 13px;
    color: var(--text-muted);
    font-weight: 600;
  }
  .devices {
    list-style: none;
    margin: 0;
    padding: 0;
  }
  .devices li {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 8px 12px;
    border-bottom: 1px solid var(--line);
  }
  .devices .name {
    font-weight: 600;
  }
  .devices button {
    margin-left: auto;
  }
  .dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--line);
    flex: none;
  }
  .dot.on {
    background: var(--success);
  }
  .approval {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    margin: 8px 0;
    padding: 10px 12px;
    border: 1px solid color-mix(in srgb, var(--accent) 45%, transparent);
    border-radius: var(--radius-control);
    background: color-mix(in srgb, var(--accent) 8%, transparent);
  }
  .actions {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    margin: 0;
  }
  button.primary,
  button.secondary,
  button.danger {
    padding: 6px 14px;
    border-radius: var(--radius-control);
    font-weight: 600;
  }
  button.primary {
    background: var(--accent);
    color: var(--on-accent);
  }
  button.secondary {
    border: 1px solid var(--line);
    background: var(--surface-2);
    color: var(--text-muted);
  }
  button.danger {
    border: 1px solid color-mix(in srgb, var(--danger) 45%, transparent);
    color: var(--danger);
  }
  .pairing {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 8px;
    text-align: center;
  }
  /* The SVG carries its own white field and quiet zone, so it scans in the
     dark theme too. */
  .qr {
    width: 240px;
    height: 240px;
    overflow: hidden;
    border-radius: var(--radius-card);
  }
  .qr :global(svg) {
    width: 100%;
    height: 100%;
  }
  .url {
    user-select: all;
    word-break: break-all;
    font-size: 12px;
  }
  .pairing .actions {
    align-self: stretch;
  }
</style>
