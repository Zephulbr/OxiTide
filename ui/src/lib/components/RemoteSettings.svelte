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
  import Button from "./settings/Button.svelte";
  import Notice from "./settings/Notice.svelte";
  import Select from "./settings/Select.svelte";
  import SettingRow from "./settings/SettingRow.svelte";
  import SettingsSection from "./settings/SettingsSection.svelte";
  import ToggleRow from "./settings/ToggleRow.svelte";

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

  let renaming = $state<{ id: string; name: string } | null>(null);

  async function commitRename() {
    if (!renaming) return;
    const { id, name } = renaming;
    renaming = null;
    const trimmed = name.trim();
    const current = devices.find((d) => d.id === id);
    if (!trimmed || trimmed === current?.name) return;
    try {
      await api.renameRemoteDevice(id, trimmed);
      devices = await api.listRemoteDevices();
    } catch (e) {
      error = errorMessage(e);
    }
  }

  function focusSelect(node: HTMLInputElement) {
    node.focus();
    node.select();
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

  const summary = $derived.by(() => {
    if (status?.listening && status.url) {
      return `Listening on ${status.url} · ${
        status.connected === 1 ? "1 phone connected" : `${status.connected} phones connected`
      }`;
    }
    if (status?.enabled && status.error) return "";
    return "Off. A phone on the same network opens a remote page — nothing to install.";
  });

  const interfaceOptions = $derived([
    { value: "", label: `Automatic${status?.address ? ` (${status.address})` : ""}` },
    ...(status?.interfaces ?? []).map((iface) => ({
      value: iface.ip,
      label: `${iface.ip} — ${iface.name}`,
    })),
  ]);
</script>

{#snippet approvals()}
  {#each status?.pending ?? [] as request (request.id)}
    <div class="approval" role="alert">
      <span>Pair <strong>{request.name}</strong> from {request.ip}?</span>
      <span class="actions">
        <Button onclick={() => resolve(request.id, false)}>Decline</Button>
        <Button variant="primary" onclick={() => resolve(request.id, true)}>Allow</Button>
      </span>
    </div>
  {/each}
{/snippet}

<SettingsSection
  title="Phone remote"
  description="Control playback from a phone on the same network. Nothing to install on the phone."
  icon="smartphone"
>
  <ToggleRow
    label="Control OxiTide from your phone"
    description={summary}
    checked={status?.enabled ?? false}
    disabled={busy || !status}
    onchange={setEnabled}
  >
    {#snippet below()}
      {#if status?.enabled && status.error && !status.listening}
        <Notice kind="warning">Not listening: {status.error}</Notice>
      {/if}
    {/snippet}
  </ToggleRow>

  {#if status?.listening}
    <SettingRow label="Pair a phone" description="Scan a QR code with the phone's camera.">
      <Button variant="primary" icon="smartphone" onclick={beginPairing}>Pair a phone</Button>
    </SettingRow>
  {/if}

  {#if (status?.pending.length ?? 0) > 0}
    <div class="approvals">{@render approvals()}</div>
  {/if}

  {#if status}
    <SettingRow
      label="Address in the QR code"
      description="Which of this PC's network addresses the phone connects to."
    >
      <Select
        label="Address in the QR code"
        width={280}
        value={status.settings.interface}
        options={interfaceOptions}
        onchange={(iface) => saveSettings({ iface })}
      />
    </SettingRow>
    <SettingRow
      label="Port"
      description={status.port && status.port !== status.settings.port
        ? `In use; listening on ${status.port}.`
        : "Between 1024 and 65526."}
    >
      <input
        class="search-input port"
        inputmode="numeric"
        aria-label="Port"
        bind:value={portText}
        onchange={commitPort}
      />
    </SettingRow>
    <ToggleRow
      label="Approve new phones on this PC"
      description="A new phone must be allowed here before it can connect."
      checked={status.settings.approve_new_devices}
      onchange={(approve) => saveSettings({ approve })}
    />
  {/if}

  <SettingRow label="Paired phones" stacked>
    {#if devices.length === 0}
      <p class="muted none">No phones paired yet.</p>
    {:else}
      <ul class="devices">
        {#each devices as device (device.id)}
          <li>
            <span class="dot" class:on={device.connected} aria-hidden="true"></span>
            {#if renaming?.id === device.id}
              <input
                class="search-input rename"
                maxlength="40"
                aria-label={`Name for ${device.name}`}
                bind:value={renaming.name}
                use:focusSelect
                onblur={commitRename}
                onkeydown={(e) => {
                  if (e.key === "Enter") void commitRename();
                  if (e.key === "Escape") renaming = null;
                }}
              />
            {:else}
              <span class="name">{device.name}</span>
              <button
                class="icon"
                aria-label={`Rename ${device.name}`}
                title="Rename"
                onclick={() => (renaming = { id: device.id, name: device.name })}
              >
                <Icon name="pencil" size={14} />
              </button>
            {/if}
            <span class="muted seen">
              {device.connected ? "Connected" : `Last seen ${lastSeenLabel(device.last_seen_ms, now)}`}
            </span>
            <Button variant="danger" aria-label={`Revoke ${device.name}`} onclick={() => revoke(device)}>
              Revoke
            </Button>
          </li>
        {/each}
      </ul>
    {/if}
  </SettingRow>

  {#if error}
    <div class="inset"><Notice kind="warning">{error}</Notice></div>
  {/if}
  <div class="foot muted">
    <p>{hint}</p>
    <p>
      The remote works on your home network only. Artwork loads from TIDAL's servers, so the phone
      needs internet access. Keep the phone screen on while you use it.
    </p>
  </div>
</SettingsSection>

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
        <Button onclick={closePairing}>Cancel</Button>
      </p>
    </div>
  </Modal>
{/if}

<style>
  .port {
    width: 8ch;
    padding: 7px 10px;
    text-align: right;
  }
  .none {
    margin: 0;
  }
  .approvals {
    padding: 10px 18px 0;
  }
  .approval {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    margin: 0 0 8px;
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
  .devices {
    margin: 0;
    padding: 0;
    list-style: none;
    border: 1px solid var(--line);
    border-radius: var(--radius-control);
  }
  .devices li {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 8px 12px;
  }
  .devices li + li {
    border-top: 1px solid var(--line);
  }
  .devices .name {
    font-weight: 600;
  }
  .seen {
    margin-left: auto;
    font-size: 12.5px;
  }
  .devices button.icon {
    display: inline-grid;
    place-items: center;
    padding: 4px;
    border-radius: var(--radius-control);
    color: var(--text-muted);
  }
  .devices button.icon:hover {
    background: var(--surface-2);
    color: var(--text);
  }
  .rename {
    width: 18ch;
    padding: 4px 8px;
  }
  .dot {
    flex: none;
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: color-mix(in srgb, var(--text) 25%, transparent);
  }
  .dot.on {
    background: var(--success);
  }
  .inset {
    padding: 14px 18px 0;
  }
  .foot {
    padding: 12px 18px 14px;
    border-top: 1px solid var(--line);
    font-size: 12.5px;
    line-height: 1.5;
  }
  .foot p {
    margin: 0;
  }
  .foot p + p {
    margin-top: 6px;
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
