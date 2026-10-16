#!/usr/bin/env python3
# SPDX-License-Identifier: AGPL-3.0-or-later
"""Regenerate the six procedural HE-AAC fixtures; stdlib plus an explicit fdkaac.

No downloads, Python packages, FFmpeg, Rust, or ignored spike files are needed.
All intermediate WAV/ADTS/MP4 files live in a TemporaryDirectory and are removed.
Only the six final MP4s and he_aac_96k_provenance.json are replaced on success.
"""

import argparse
import hashlib
import json
import math
import os
from pathlib import Path
import struct
import subprocess
import tempfile
import wave

HERE = Path(__file__).resolve().parent
RATE = 48000
CORE_RATE = 24000
N = RATE * 3
SPLIT = 24
SOURCE_SHA = "54f2858ae60124f30fd37c77058582450d9c35d15ceaa808202df87e87f65726"
# Hash [little-endian u32 AU length][AU bytes] for every packet, in order.
# These pin packet boundaries as well as unchanged AAC bytes from the original spike.
ACCESS_UNIT_SHA = {
    5: "13af5b16761ca7554f9c8ce239e56a563217a4e6cbf5a283c623fadfc48229cf",
    29: "5c6f1754ca72bcf793bff1fcb873c31cd31aaf7db9925c8ffe9e3828c3d3fb1b",
}


def sha(data):
    return hashlib.sha256(data).hexdigest()


def source(path):
    state = 0x12345678
    pcm = bytearray()
    for i in range(N):
        t = i / RATE
        state ^= (state << 13) & 0xffffffff
        state ^= state >> 17
        state ^= (state << 5) & 0xffffffff
        noise = state / 0xffffffff * 2 - 1
        envelope = min(1, t * 100, (3 - t) * 100)
        attack = math.exp(-((t * 4) % 1) * 35)
        chirp = math.sin(2 * math.pi * (1100 * t + 650 * t * t))
        left = (0.16 * math.sin(2*math.pi*440*t) + 0.08*math.sin(2*math.pi*6000*t)
                + 0.11*math.sin(2*math.pi*14000*t) + 0.04*chirp + 0.09*attack*noise)
        right = (0.16 * math.sin(2*math.pi*660*t) + 0.08*math.sin(2*math.pi*7500*t)
                 + 0.11*math.sin(2*math.pi*16000*t) - 0.04*chirp - 0.09*attack*noise)
        pcm.extend(struct.pack("<hh", round(left*envelope*32767), round(right*envelope*32767)))
    with wave.open(str(path), "wb") as w:
        w.setparams((2, 2, RATE, 0, "NONE", "not compressed"))
        w.writeframes(pcm)
    return sha(path.read_bytes())


def adts_packets(data):
    packets = []
    pos = 0
    channels = None
    while pos < len(data):
        h = data[pos:pos+7]
        if len(h) != 7 or h[0] != 0xff or h[1] & 0xf6 != 0xf0:
            raise ValueError("invalid ADTS header")
        size = ((h[3] & 3) << 11) | (h[4] << 3) | (h[5] >> 5)
        current_channels = ((h[2] & 1) << 2) | (h[3] >> 6)
        if (h[6] & 3 or not h[1] & 1 or size < 7
                or (h[2] >> 2) & 15 != 6 or h[2] >> 6 != 1):
            raise ValueError("expected unprotected, single-block AAC-LC 24 kHz core")
        if channels is not None and current_channels != channels:
            raise ValueError("ADTS channel configuration changed")
        channels = current_channels
        packet = data[pos+7:pos+size]
        if len(packet) != size - 7:
            raise ValueError("truncated ADTS packet")
        packets.append(packet)
        pos += size
    return packets, channels


def asc(profile, channels, signaling):
    def bits(value, n):
        return format(value, "0%db" % n)
    if signaling == "hierarchical":
        s = bits(profile, 5) + bits(6, 4) + bits(channels, 4) + bits(3, 4) + bits(2, 5) + "000"
    else:
        s = bits(2, 5) + bits(6, 4) + bits(channels, 4) + "000"
        if signaling == "sync":
            s += bits(0x2b7, 11) + bits(5, 5) + "1" + bits(3, 4)
            if profile == 29:
                s += bits(0x548, 11) + "1"
    s += "0" * (-len(s) % 8)
    return int(s, 2).to_bytes(len(s)//8, "big")


def u32(*values):
    return struct.pack(">" + "I"*len(values), *values)


def box(kind, payload):
    return u32(len(payload)+8) + kind.encode("ascii") + payload


def full(kind, payload, flags=0):
    return box(kind, u32(flags) + payload)


def desc(tag, payload):
    n = len(payload)
    encoded = [n & 127]
    n >>= 7
    while n:
        encoded.insert(0, 128 | (n & 127))
        n >>= 7
    return bytes([tag, *encoded]) + payload


def init_segment(config, core_channels):
    matrix = u32(0x10000, 0, 0, 0, 0x10000, 0, 0, 0, 0x40000000)
    ftyp = box("ftyp", b"iso6" + u32(0x200) + b"iso6isommp41")
    decoder = desc(4, b"\x40\x15" + b"\0"*3 + u32(96000, 96000) + desc(5, config))
    esds = full("esds", desc(3, b"\0\1\0" + decoder + desc(6, b"\x02")))
    sample = box("mp4a", b"\0"*6 + struct.pack(">H", 1) + b"\0"*8
                 + struct.pack(">HHHHI", core_channels, 16, 0, 0, CORE_RATE << 16) + esds)
    # Fragment samples are described by trun/trex, not the init's empty tables.
    stbl = box("stbl", full("stsd", u32(1)+sample)
               + full("stts", u32(0)) + full("stsc", u32(0))
               + full("stsz", u32(0, 0)) + full("stco", u32(0)))
    dinf = box("dinf", full("dref", u32(1)+full("url ", b"", 1)))
    minf = box("minf", full("smhd", b"\0"*4)+dinf+stbl)
    mdhd = full("mdhd", u32(0, 0, CORE_RATE, 0)+struct.pack(">HH", 0x55c4, 0))
    hdlr = full("hdlr", u32(0)+b"soun"+b"\0"*12+b"Synthetic FDK AAC\0")
    mdia = box("mdia", mdhd+hdlr+minf)
    tkhd = full("tkhd", u32(0, 0, 1, 0, 0)+b"\0"*8
                + struct.pack(">HHHH", 0, 0, 0x100, 0)+matrix+u32(0, 0), 7)
    mvhd = full("mvhd", u32(0, 0, CORE_RATE, 0, 0x10000)
                + struct.pack(">HH", 0x100, 0)+b"\0"*8+matrix+b"\0"*24+u32(2))
    # Track 1, description 1, 1024 core-clock ticks/AU; sizes come from trun.
    mvex = box("mvex", full("trex", u32(1, 1, 1024, 0, 0)))
    return ftyp + box("moov", mvhd+box("trak", tkhd+mdia)+mvex)


def fragment(packets, sequence, first_packet):
    tfhd = full("tfhd", u32(1), 0x020000)  # default-base-is-moof
    tfdt = full("tfdt", struct.pack(">Q", first_packet*1024), 0x01000000)

    def moof(offset):
        trun = full("trun", u32(len(packets)) + struct.pack(">i", offset)
                    + u32(*(len(p) for p in packets)), 0x000201)
        return box("moof", full("mfhd", u32(sequence))+box("traf", tfhd+tfdt+trun))

    header = moof(0)
    header = moof(len(header)+8)  # data_offset relative to this moof, past mdat header
    return header + box("mdat", b"".join(packets)), len(header)


def fragmented_mp4(packets, config, core_channels):
    if len(packets) <= SPLIT:
        raise ValueError("two-fragment fixture needs more than 24 packets")
    init = init_segment(config, core_channels)
    output = bytearray(init)
    fragments = []
    for sequence, (start, stop) in enumerate([(0, SPLIT), (SPLIT, len(packets))], 1):
        data, moof_bytes = fragment(packets[start:stop], sequence, start)
        offset = len(output)
        fragments.append({"sequence": sequence, "offset": offset, "bytes": len(data),
                          "end_exclusive": offset+len(data), "moof_bytes": moof_bytes,
                          "mdat_offset": offset+moof_bytes, "first_packet": start,
                          "packets": stop-start, "decode_time_ticks": start*1024,
                          "duration_ticks": (stop-start)*1024,
                          "decoded_frames": (stop-start)*2048})
        output.extend(data)
    return bytes(output), {"init": {"offset": 0, "bytes": len(init), "end_exclusive": len(init)},
                           "fragments": fragments}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--encoder", required=True, type=Path, help="path to an existing fdkaac executable")
    args = parser.parse_args()
    encoder = args.encoder.resolve(strict=True)
    manifest = {
        "source": {"generator": "crates/oxitide-audio/tests/fixtures/generate_he_aac.py",
                   "generator_sha256": sha(Path(__file__).read_bytes()), "sha256": SOURCE_SHA,
                   "sample_rate": RATE, "channels": 2, "frames": N, "seed": "0x12345678"},
        "encoder": {"reference_build": "fdkaac 1.0.5 / FDK AAC 2.0.2 (per RareWares)",
                    "download_url": "https://www.rarewares.org/files/aac/fdkaac-1.0.5-x64.zip",
                    "archive_sha256": "2717cf55c641326cd7551f337ca5b2220764ce21aea609b24dd3d52755f32d4a",
                    "exe_sha256": sha(encoder.read_bytes())},
        "container": {"format": "fragmented MP4", "top_level_boxes": ["ftyp", "moov", "moof", "mdat", "moof", "mdat"],
                      "track_id": 1, "time_base": "1/24000", "default_sample_duration_ticks": 1024,
                      "split_after_packets": SPLIT, "edit_list": False, "gapless_trimming": False},
        "fixtures": [],
    }
    # Even staging outputs live here, so interrupted/failed encodes cannot leave WAV/ADTS files behind.
    with tempfile.TemporaryDirectory(prefix="he-aac-", dir=HERE) as temporary:
        work = Path(temporary)
        wav = work / "synthetic-stereo-48k.wav"
        if source(wav) != SOURCE_SHA:
            raise RuntimeError("source WAV differs from the original golden signal")
        for profile, version, count in [(5, "v1", 73), (29, "v2", 74)]:
            adts = work / ("he-aac-"+version+"-96k.aac")
            options = ["-p", str(profile), "-b", "96000", "-s", "2", "-f", "2", "--no-timestamp"]
            run = subprocess.run([str(encoder), *options, "-o", str(adts), str(wav)],
                                 stdin=subprocess.DEVNULL, capture_output=True, text=True, timeout=30)
            if run.returncode:
                raise RuntimeError("fdkaac failed: " + run.stderr)
            packets, channels = adts_packets(adts.read_bytes())
            framed = b"".join(struct.pack("<I", len(p))+p for p in packets)
            if (len(packets) != count or channels != (2 if profile == 5 else 1)
                    or sha(framed) != ACCESS_UNIT_SHA[profile]):
                raise RuntimeError("encoder did not reproduce the original AAC access units")
            payload = b"".join(packets)
            for signaling in ["hierarchical", "sync", "implicit"]:
                name = "he_aac_"+version+"_96k_"+signaling+".mp4"
                config = asc(profile, channels, signaling)
                data, layout = fragmented_mp4(packets, config, channels)
                (work/name).write_bytes(data)
                manifest["fixtures"].append({
                    "path": "crates/oxitide-audio/tests/fixtures/"+name,
                    "sha256": sha(data), "bytes": len(data), "profile": profile, "signaling": signaling,
                    "asc_hex": config.hex(), "core_rate": CORE_RATE, "core_channels": channels,
                    "output_rate": RATE, "output_channels": 2, "packets": len(packets),
                    "decoded_frames": len(packets)*2048, "access_units_sha256": sha(framed),
                    "packet_payload_sha256": sha(payload),
                    "payload_bitrate": len(payload)*8/(len(packets)*2048/RATE),
                    "encoder_args": options, "layout": layout,
                })
        provenance = "he_aac_96k_provenance.json"
        with (work/provenance).open("w", encoding="utf-8", newline="\n") as out:
            out.write(json.dumps(manifest, indent=2)+"\n")
        for name in [Path(f["path"]).name for f in manifest["fixtures"]]+[provenance]:
            os.replace(work/name, HERE/name)
    for f in manifest["fixtures"]:
        print(Path(f["path"]).name, f["sha256"], json.dumps(f["layout"]))


if __name__ == "__main__":
    main()
