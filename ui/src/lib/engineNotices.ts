import { sentenceCase } from "./format";
import { t, type MessageKey } from "./i18n.svelte";

/// The audio engine's fixed output notices (`PlaybackState.bit_perfect_notice`),
/// matched by exact text (v1.1 i18n spec §10.2). The wire and the Rust
/// constants stay English, so the Rust comparisons that rely on them keep
/// working; `engineNotices.test.ts` pins this table against the Rust sources.
export const ENGINE_NOTICES: Readonly<Record<string, MessageKey>> = {
  "selected output device unavailable; using system default": "notice.deviceUnavailable",
  "exclusive mode unavailable; using shared output": "notice.exclusiveUnavailable",
  "volume applied in software; bit-perfect compromised": "notice.softwareVolume",
  "output device disconnected": "notice.deviceDisconnected",
  "ASIO driver was reset or disconnected": "notice.asioLost",
};

/// The one composed notice: `ASIO unavailable (<reason>); using shared output`
/// (`routed_output.rs`). The reason is a driver detail and stays English.
const ASIO_UNAVAILABLE = /^ASIO unavailable \((.*)\); using shared output$/;

/// A notice in the UI language; anything unknown shows as before (English,
/// sentence case).
export function engineNotice(text: string): string {
  const key = ENGINE_NOTICES[text];
  if (key) return t(key);
  const asio = ASIO_UNAVAILABLE.exec(text);
  if (asio) return t("notice.asioUnavailable", { reason: asio[1] });
  return sentenceCase(text);
}
