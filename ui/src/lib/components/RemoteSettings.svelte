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
  import { copyText } from "../clipboard";
  import { countdownLabel, firewallHint, firewallNotice, lastSeenLabel, parsePort } from "../remoteSettings";
  import Icon from "./Icon.svelte";
  import Modal from "./Modal.svelte";
  import Button from "./settings/Button.svelte";
  import Notice from "./settings/Notice.svelte";
  import Select from "./settings/Select.svelte";
  import SettingRow from "./settings/SettingRow.svelte";
  import SettingsSection from "./settings/SettingsSection.svelte";
  import ToggleRow from "./settings/ToggleRow.svelte";
  import { t, tSplit } from "../i18n.svelte";

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
      error = t("remoteSettings.portRange", { min: 1024, max: 65526 });
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

  const port = $derived(status?.port ?? status?.settings.port ?? 47820);
  const hint = $derived(firewallHint(navigator.userAgent, port));
  const firewall = $derived(status?.enabled ? firewallNotice(status.firewall ?? null, port) : null);
  let commandCopied = $state(false);

  async function copyCommand(command: string) {
    commandCopied = await copyText(command);
  }

  const summary = $derived.by(() => {
    if (status?.listening && status.url) {
      return t("remoteSettings.listening", { url: status.url, count: status.connected });
    }
    if (status?.enabled && status.error) return "";
    return t("remoteSettings.off");
  });

  const interfaceOptions = $derived([
    { value: "", label: status?.address ? t("remoteSettings.autoAddress", { address: status.address }) : t("device.icon.auto") },
    ...(status?.interfaces ?? []).map((iface) => ({
      value: iface.ip,
      label: `${iface.ip} — ${iface.name}`,
    })),
  ]);
</script>

{#snippet approvals()}
  {#each status?.pending ?? [] as request (request.id)}
    {@const [before, after] = tSplit("remoteSettings.pairRequest", "name", { ip: request.ip })}
    <div class="approval" role="alert">
      <span>{before}<strong>{request.name}</strong>{after}</span>
      <span class="actions">
        <Button onclick={() => resolve(request.id, false)}>{t("remoteSettings.decline")}</Button>
        <Button variant="primary" onclick={() => resolve(request.id, true)}>{t("remoteSettings.allow")}</Button>
      </span>
    </div>
  {/each}
{/snippet}

<SettingsSection
  title={t("settings.section.phoneRemote")}
  description={t("settings.section.phoneRemote.description")}
  icon="smartphone"
>
  <ToggleRow
    id="remote-enable" label={t("settings.remoteEnable.label")}
    description={summary}
    checked={status?.enabled ?? false}
    disabled={busy || !status}
    onchange={setEnabled}
  >
    {#snippet below()}
      {#if status?.enabled && status.error && !status.listening}
        <Notice kind="warning">{t("remoteSettings.notListening", { error: status.error })}</Notice>
      {/if}
    {/snippet}
  </ToggleRow>

  {#if status?.listening}
    <SettingRow id="remote-pair" label={t("settings.remotePair.label")} description={t("settings.remotePair.description")}>
      <Button variant="primary" icon="smartphone" onclick={beginPairing}>{t("settings.remotePair.label")}</Button>
    </SettingRow>
  {/if}

  {#if (status?.pending.length ?? 0) > 0}
    <div class="approvals">{@render approvals()}</div>
  {/if}

  {#if status}
    <SettingRow
      id="remote-address" label={t("settings.remoteAddress.label")}
      description={t("settings.remoteAddress.description")}
    >
      <Select
        label={t("settings.remoteAddress.label")}
        width={280}
        value={status.settings.interface}
        options={interfaceOptions}
        onchange={(iface) => saveSettings({ iface })}
      />
    </SettingRow>
    <SettingRow
      id="remote-port" label={t("settings.remotePort.label")}
      description={status.port && status.port !== status.settings.port
        ? t("remoteSettings.portInUse", { port: status.port })
        : t("remoteSettings.portHelp", { min: 1024, max: 65526 })}
    >
      <input
        class="search-input port"
        inputmode="numeric"
        aria-label={t("settings.remotePort.label")}
        bind:value={portText}
        onchange={commitPort}
      />
    </SettingRow>
    <ToggleRow
      id="remote-approve" label={t("settings.remoteApprove.label")}
      description={t("settings.remoteApprove.description")}
      checked={status.settings.approve_new_devices}
      onchange={(approve) => saveSettings({ approve })}
    />
  {/if}

  <SettingRow id="remote-devices" label={t("settings.remoteDevices.label")} stacked>
    {#if devices.length === 0}
      <p class="muted none">{t("remoteSettings.noDevices")}</p>
    {:else}
      <ul class="devices">
        {#each devices as device (device.id)}
          <li>
            <span class="dot" class:on={device.connected} aria-hidden="true"></span>
            {#if renaming?.id === device.id}
              <input
                class="search-input rename"
                maxlength="40"
                aria-label={t("remoteSettings.nameFor", { name: device.name })}
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
                aria-label={t("remoteSettings.renameNamed", { name: device.name })}
                title={t("menu.rename")}
                onclick={() => (renaming = { id: device.id, name: device.name })}
              >
                <Icon name="pencil" size={14} />
              </button>
            {/if}
            <span class="muted seen">
              {device.connected ? t("remoteSettings.connected") : t("remoteSettings.lastSeen", { when: lastSeenLabel(device.last_seen_ms, now) })}
            </span>
            <Button variant="danger" aria-label={t("remoteSettings.revokeNamed", { name: device.name })} onclick={() => revoke(device)}>
              {t("remoteSettings.revoke")}
            </Button>
          </li>
        {/each}
      </ul>
    {/if}
  </SettingRow>

  {#if error}
    <div class="inset"><Notice kind="warning">{error}</Notice></div>
  {/if}
  {#if firewall && !firewall.ok && firewall.command}
    {@const command = firewall.command}
    <div class="inset">
      <Notice kind="warning">
        <p class="fw-text">{firewall.text}</p>
        <div class="fw-command">
          <code>{command}</code>
          <Button onclick={() => copyCommand(command)}>{commandCopied ? t("remoteSettings.copied") : t("remoteSettings.copy")}</Button>
        </div>
      </Notice>
    </div>
  {/if}
  <div class="foot muted">
    <p>{firewall?.ok ? firewall.text : hint}</p>
    <p>{t("remoteSettings.footer")}</p>
  </div>
</SettingsSection>

{#if pairing}
  <Modal title={t("settings.remotePair.label")} onclose={closePairing}>
    <div class="pairing">
      <!-- The SVG is generated by OxiTide itself (qrcode crate), not remote input. -->
      <div class="qr" aria-label={t("remoteSettings.qr")} role="img">{@html pairing.qr_svg}</div>
      <p>{t("remoteSettings.scan")}</p>
      <code class="url">{pairing.url}</code>
      <p class="muted">
        {t("remoteSettings.expires", { time: countdownLabel(remaining) })}
      </p>
      {@render approvals()}
      <p class="actions">
        <Button onclick={closePairing}>{t("common.cancel")}</Button>
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
  .fw-text {
    margin: 0 0 8px;
  }
  .fw-command {
    display: flex;
    align-items: center;
    gap: 8px;
    flex-wrap: wrap;
  }
  .fw-command code {
    user-select: all;
    word-break: break-all;
    font-size: 12px;
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
