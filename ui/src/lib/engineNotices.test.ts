import { describe, expect, it } from "vitest";
import alsaOutput from "../../../crates/oxitide-audio/src/alsa_output.rs?raw";
import asioOutput from "../../../crates/oxitide-audio/src/asio_output.rs?raw";
import engine from "../../../crates/oxitide-audio/src/engine.rs?raw";
import output from "../../../crates/oxitide-audio/src/output.rs?raw";
import routedOutput from "../../../crates/oxitide-audio/src/routed_output.rs?raw";
import wasapiOutput from "../../../crates/oxitide-audio/src/wasapi_output.rs?raw";
import { ENGINE_NOTICES, engineNotice } from "./engineNotices";

describe("engine notices", () => {
  it("matches text the audio engine still emits", () => {
    // The Rust sources that emit the notices, read as text.
    const sources = [alsaOutput, wasapiOutput, asioOutput, output, engine].join("\n");
    for (const text of Object.keys(ENGINE_NOTICES)) {
      expect(sources, `"${text}" no longer appears in crates/oxitide-audio`).toContain(`"${text}"`);
    }
    expect(routedOutput).toContain('"ASIO unavailable ({reason}); using shared output"');
  });

  it("translates known notices and keeps unknown ones as they were", () => {
    expect(engineNotice("exclusive mode unavailable; using shared output")).toBe(
      "Exclusive mode unavailable; using shared output",
    );
    expect(engineNotice("ASIO unavailable (ASIO driver does not support 96 kHz); using shared output")).toBe(
      "ASIO unavailable (ASIO driver does not support 96 kHz); using shared output",
    );
    expect(engineNotice("something new; still shown")).toBe("Something new; still shown");
  });
});
