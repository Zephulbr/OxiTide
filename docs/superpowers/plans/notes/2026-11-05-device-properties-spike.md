# Device properties spike (output picker, plan Task 1)

- **Date:** 2026-11-05
- **Machine:** owner's Windows 11 desktop (MSI board, AMD GPU)
- **Plan:** `plans/2026-11-05-output-device-picker.md`, Task 1. This settles spec §6.1 and
  risk §11 (WASAPI transport).
- **Method:** a throwaway example dumped every property in each active render endpoint's
  `IPropertyStore`, then timed `IsFormatSupported(EXCLUSIVE)` over 8 rates × 3 integer
  depths. The example was deleted afterwards.

## Properties

| Endpoint | `DeviceDesc` (endpoint) | `DeviceInterface_FriendlyName` (adapter) | `Device_EnumeratorName` | `FormFactor` |
|---|---|---|---|---|
| LG TV over HDMI | `4 - LG TV` | AMD High Definition Audio Device | `HDAUDIO` | 9 (DigitalAudioDisplayDevice) |
| Monitor over DP | `1 - C27JG5x` | AMD High Definition Audio Device | `HDAUDIO` | 9 |
| Motherboard optical | `Realtek Digital Output` | Realtek(R) Audio | `HDAUDIO` | 8 (SPDIF) |
| USB mic headphone out | `Speakers` | HyperX DuoCast | `USB` | 1 (Speakers) |
| USB DAC (Thesycon driver) | `DX5` | TOPPING USB DAC | `TUSBAUDIO_ENUM` | 1 (Speakers) |

- **Enumerator name.** `PKEY_Device_EnumeratorName` `{A45C254E-…},24` is on the endpoint
  store, so no adapter lookup is needed. Vendor USB drivers use their own enumerator
  (`TUSBAUDIO_ENUM`), so "contains `USB`" is the rule. Bluetooth is `BTHENUM` / `BTHHFENUM`
  / `BTHLEDEVICE` (not present here).
- **Form factor.** `PKEY_AudioEndpoint_FormFactor` `{1DA5D803-…},0` is present on every
  endpoint and is reliable for HDMI/DP (9) and S/PDIF (8).
- **Windows numbers HDMI/DP endpoints** (`4 - LG TV`), so the clean name strips a leading
  `\d+ - ` as well as the `\d+- ` instance prefix.
- **Endpoint vs adapter name:** the endpoint name is the better label unless it's a
  generic role word. "DX5" beats "TOPPING USB DAC", while "HyperX DuoCast" beats
  "Speakers". So the rule is: generic endpoint name (Speakers, Headphones, Headset, Line,
  Digital Output, S/PDIF, …) on a non-internal device → adapter name; otherwise the
  endpoint name.

## Exclusive format check

| Endpoint | Exclusive formats found | Time |
|---|---|---|
| LG TV | 192/96/48/44.1 kHz at 24-bit | 77 ms |
| Realtek Digital Output | up to 192 kHz, 24/16-bit | 17 ms |
| HyperX DuoCast | none (shared only) | 21 ms |
| C27JG5x | 48/44.1 kHz at 24-bit | 22 ms |
| TOPPING DX5 | up to 384 kHz, 32/24-bit | 18 ms |

The whole grid takes about 20–80 ms per device, so the checks run on a worker thread after
the list is returned (spec §7) and never block the command.
