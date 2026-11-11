#!/usr/bin/env node
// Output-level probe (Linux): measures the volume OxiTide's output path applies
// without listening. No dependencies; needs `amixer` (alsa-utils), and
// `pw-dump` for the PipeWire part, `arecord` for `record`.
//
//   node scripts/audio/level-probe.mjs snapshot [--device hw:CARD=Pro,DEV=0] [--volume 0.5]
//       The gain chain right now: app slider -> software gain, the card's
//       hardware mixer (read back in dB), PipeWire's software volume, who has
//       the PCM open, and the total for the current path and the other paths
//       the picker offers for the same card. Run it in each state you compare.
//       Exclusive output plays at 100% and never sets the mixer (spec
//       2026-11-05-oxitide-exclusive-fixed-volume), so with the slider at 100%
//       every path should total the same.
//   node scripts/audio/level-probe.mjs record --capture hw:1,0 --seconds 15 --out a.wav
//       Record the DAC's analog output through a line/mic input (arecord).
//   node scripts/audio/level-probe.mjs compare a.wav b.wav
//       Level difference of b against a: the two recordings are aligned on
//       their loudness envelopes, then compared frame by frame (median dB).
//
// Defaults come from ~/.config/oxitide/config.toml ([audio] output_device,
// exclusive, volume).

import { execFileSync, spawnSync } from 'node:child_process';
import { readFileSync, existsSync, readlinkSync } from 'node:fs';
import { homedir } from 'node:os';
import { join } from 'node:path';

// Must match crates/oxitide-audio/src/output.rs VOLUME_TAPER_EXPONENT.
const TAPER = 1.661;

const softwareDb = (position) =>
  position <= 0 ? -Infinity : 20 * Math.log10(Math.min(1, position) ** TAPER);

const fmt = (db) => (db === -Infinity ? '  -inf dB' : `${db >= 0 ? ' ' : ''}${db.toFixed(2)} dB`);

function args(argv) {
  const out = { _: [] };
  for (let i = 0; i < argv.length; i++) {
    const a = argv[i];
    if (a.startsWith('--')) {
      const next = argv[i + 1];
      if (next === undefined || next.startsWith('--')) out[a.slice(2)] = true;
      else out[a.slice(2)] = argv[++i];
    } else out._.push(a);
  }
  return out;
}

function run(cmd, argv) {
  try {
    return execFileSync(cmd, argv, { encoding: 'utf8', stdio: ['ignore', 'pipe', 'ignore'] });
  } catch {
    return null;
  }
}

// --- app config -------------------------------------------------------------

function appConfig() {
  const path = join(homedir(), '.config', 'oxitide', 'config.toml');
  if (!existsSync(path)) return {};
  const text = readFileSync(path, 'utf8');
  const section = text.split(/^\[/m).find((s) => s.startsWith('audio]')) ?? '';
  const get = (key) => section.match(new RegExp(`^${key}\\s*=\\s*(.+)$`, 'm'))?.[1]?.trim();
  const str = (v) => v?.replace(/^"(.*)"$/, '$1');
  return {
    device: str(get('output_device')),
    exclusive: get('exclusive') === 'true',
    volume: get('volume') !== undefined ? Number(get('volume')) : undefined,
  };
}

// --- ALSA -------------------------------------------------------------------

/** Card id/index from a PCM name: hw:CARD=Pro,DEV=0, plughw:3,0, sysdefault:CARD=Pro. */
function cardOf(device) {
  const m = device?.match(/^(?:hw|plughw|sysdefault|front|iec958|dmix):(?:CARD=)?([^,]+)/);
  return m?.[1];
}

function cardIndex(card) {
  if (card === undefined) return undefined;
  if (/^\d+$/.test(card)) return Number(card);
  try {
    return Number(readlinkSync(`/proc/asound/${card}`).replace(/^card/, ''));
  } catch {
    return undefined;
  }
}

function cardName(index) {
  const cards = readFileSync('/proc/asound/cards', 'utf8');
  const m = cards.match(new RegExp(`^\\s*${index} \\[([^\\]]+?)\\s*\\]: .+? - (.+)$`, 'm'));
  return m ? { id: m[1].trim(), name: m[2].trim() } : { id: String(index), name: '?' };
}

/** The element OxiTide writes (Master, else PCM), as ALSA reports it. */
function hardwareMixer(index) {
  for (const name of ['Master', 'PCM']) {
    const text = run('amixer', ['-c', String(index), 'sget', name]);
    if (!text || !/pvolume/.test(text)) continue;
    const limits = text.match(/Limits: Playback (-?\d+) - (-?\d+)/);
    const channels = [...text.matchAll(/^\s+(Front Left|Front Right|Mono): Playback (-?\d+) \[(\d+)%\](?: \[(-?[\d.]+)dB\])?(?: \[(on|off)\])?/gm)].map(
      (m) => ({ channel: m[1], raw: Number(m[2]), db: m[4] !== undefined ? Number(m[4]) : undefined, on: m[5] !== 'off' }),
    );
    const contents = run('amixer', ['-c', String(index), 'contents']) ?? '';
    const block = contents.split(/^numid=/m).find((b) => b.includes(`name='${name} Playback Volume'`)) ?? '';
    const range = block.match(/dBminmax-min=(-?[\d.]+)dB,max=(-?[\d.]+)dB/) ?? block.match(/dBscale-min=(-?[\d.]+)dB,step=([\d.]+)dB/);
    let dbRange;
    if (range && block.includes('dBminmax')) dbRange = { min: Number(range[1]), max: Number(range[2]) };
    else if (range && limits) {
      const steps = Number(limits[2]) - Number(limits[1]);
      dbRange = { min: Number(range[1]), max: Number(range[1]) + Number(range[2]) * steps };
    }
    return {
      name,
      min: limits ? Number(limits[1]) : undefined,
      max: limits ? Number(limits[2]) : undefined,
      channels,
      dbRange,
      linearDb: /dBscale|dBminmax/.test(block),
    };
  }
  return undefined;
}

/** Who has the card's first playback PCM open, and in what format. */
function pcmState(index) {
  const out = [];
  for (const dev of ['pcm0p']) {
    const base = `/proc/asound/card${index}/${dev}/sub0`;
    if (!existsSync(base)) continue;
    const status = readFileSync(`${base}/status`, 'utf8').trim();
    const params = readFileSync(`${base}/hw_params`, 'utf8').trim();
    if (status === 'closed') {
      out.push('closed');
      continue;
    }
    const pid = status.match(/owner_pid\s*:\s*(\d+)/)?.[1];
    let owner = '?';
    if (pid) {
      try {
        owner = readFileSync(`/proc/${pid}/comm`, 'utf8').trim();
      } catch {}
    }
    const state = status.match(/state:\s*(\S+)/)?.[1] ?? '?';
    const format = params.match(/format:\s*(\S+)/)?.[1];
    const rate = params.match(/rate:\s*(\d+)/)?.[1];
    out.push(`${state} by ${owner}${pid ? ` (pid ${pid})` : ''}, ${format} @ ${rate} Hz`);
  }
  return out.join('; ') || 'unknown';
}

// --- PipeWire ---------------------------------------------------------------

function pipewireSink(index) {
  const text = run('pw-dump', []);
  if (!text) return undefined;
  let objects;
  try {
    objects = JSON.parse(text);
  } catch {
    return undefined;
  }
  for (const o of objects) {
    const p = o.info?.props ?? {};
    if (p['media.class'] !== 'Audio/Sink' || Number(p['alsa.card'] ?? p['api.alsa.pcm.card']) !== index) continue;
    const props = (o.info.params?.Props ?? []).find((x) => x.channelVolumes) ?? {};
    const soft = props.softVolumes ?? [1];
    const softLinear = Math.max(...soft);
    return {
      id: o.id,
      name: p['node.description'] ?? p['node.name'],
      volume: props.channelVolumes ? Math.max(...props.channelVolumes) : undefined,
      softDb: softLinear > 0 ? 20 * Math.log10(softLinear) : -Infinity,
      mute: Boolean(props.mute || props.softMute),
      state: o.info.state,
    };
  }
  return undefined;
}

// --- commands ---------------------------------------------------------------

function snapshot(opts) {
  const config = appConfig();
  const device = opts.device ?? config.device ?? 'default';
  const volume = opts.volume !== undefined ? Number(opts.volume) : config.volume ?? 1;
  const exclusive = opts.mode ? opts.mode === 'exclusive' : config.exclusive;
  const index = cardIndex(opts.card ?? cardOf(device));
  console.log(`app      device ${device}, ${exclusive ? 'exclusive' : 'shared'}, slider ${(volume * 100).toFixed(0)}%` +
    `${opts.volume === undefined ? ' (from config.toml; pass --volume if the slider moved since it was saved)' : ''}`);
  if (index === undefined) {
    console.log('card     unknown: pass --card <id or index> (the card behind the default device)');
    return;
  }
  const { id, name } = cardName(index);
  const mixer = hardwareMixer(index);
  const hwDb = mixer?.channels.length ? Math.max(...mixer.channels.map((c) => c.db ?? 0)) : 0;
  const hwOn = mixer ? mixer.channels.every((c) => c.on) : true;
  console.log(`card     ${index} [${id}] ${name}`);
  console.log(`pcm      ${pcmState(index)}`);
  if (mixer) {
    const ch = mixer.channels.map((c) => `${c.channel} ${c.raw}/${mixer.max} ${fmt(c.db ?? NaN).trim()}${c.on ? '' : ' MUTED'}`).join(', ');
    console.log(`mixer    '${mixer.name}' ${ch}${mixer.dbRange ? ` (range ${mixer.dbRange.min}..${mixer.dbRange.max} dB)` : ''}`);
  } else console.log('mixer    no writable Master/PCM element (exclusive uses software volume)');
  const sink = pipewireSink(index);
  if (sink) console.log(`pipewire sink ${sink.id} "${sink.name}" ${sink.state}: volume ${sink.volume?.toFixed(2)}, software part ${fmt(sink.softDb).trim()}${sink.mute ? ' MUTED' : ''}`);

  const sw = softwareDb(volume);
  const pw = sink?.softDb ?? 0;
  const direct = device.startsWith('hw:');
  const rows = [];
  // Exclusive is fixed at 100% on a card with its own control; without one
  // (fallback allowed) the app's volume applies in software.
  if (mixer) rows.push({ path: 'exclusive on hw: (fixed 100%)', sw: 0, pw: null, hw: hwDb });
  else rows.push({ path: 'exclusive on hw: (software volume)', sw, pw: null, hw: 0 });
  rows.push({ path: 'shared plughw:/sysdefault:', sw, pw: null, hw: hwDb });
  if (sink) rows.push({ path: 'default (PipeWire)', sw, pw, hw: hwDb });
  const current = exclusive && direct ? 0 : direct || /^(plughw|sysdefault):/.test(device) ? 1 : 2;

  console.log('\npath                                 app sw     pipewire   hw mixer   total');
  rows.forEach((r, i) => {
    const total = r.sw + (r.pw ?? 0) + r.hw + (hwOn ? 0 : -Infinity);
    console.log(`${i === current ? '>' : ' '} ${r.path.padEnd(35)}${fmt(r.sw).padStart(9)}  ${(r.pw === null ? 'bypassed' : fmt(r.pw)).padStart(9)}  ${fmt(r.hw).padStart(9)}  ${fmt(total).padStart(9)}`);
  });
  if (mixer) console.log('\nThe hw mixer is the system volume for this card (PipeWire drives it); exclusive leaves it as it is.');
}

function record(opts) {
  if (!opts.capture || !opts.out) throw new Error('usage: record --capture <alsa pcm, e.g. hw:1,0> --out file.wav [--seconds 15] [--rate 48000]');
  const seconds = String(opts.seconds ?? 15);
  const rate = String(opts.rate ?? 48000);
  const r = spawnSync('arecord', ['-D', opts.capture, '-f', 'S32_LE', '-c', '2', '-r', rate, '-d', seconds, opts.out], { stdio: 'inherit' });
  if (r.status !== 0) {
    // Many inputs refuse S32/stereo; let plughw: convert.
    console.error('arecord failed; retrying through plughw: conversion');
    const plug = opts.capture.replace(/^hw:/, 'plughw:');
    spawnSync('arecord', ['-D', plug, '-f', 'S32_LE', '-c', '2', '-r', rate, '-d', seconds, opts.out], { stdio: 'inherit' });
  }
  const level = levels(readWav(opts.out));
  console.log(`${opts.out}: rms ${fmt(level.rmsDb).trim()} FS, peak ${fmt(level.peakDb).trim()} FS${level.peakDb > -0.5 ? '  CLIPPING: lower the input gain and record both again' : ''}`);
}

// --- WAV analysis -----------------------------------------------------------

export function readWav(path) {
  const buf = readFileSync(path);
  if (buf.toString('ascii', 0, 4) !== 'RIFF' || buf.toString('ascii', 8, 12) !== 'WAVE') throw new Error(`${path}: not a WAV file`);
  let pos = 12;
  let fmtChunk;
  let data;
  while (pos + 8 <= buf.length) {
    const idChunk = buf.toString('ascii', pos, pos + 4);
    let size = buf.readUInt32LE(pos + 4);
    const body = pos + 8;
    if (idChunk === 'data' && (size === 0 || size === 0xffffffff || body + size > buf.length)) size = buf.length - body;
    if (idChunk === 'fmt ') fmtChunk = buf.subarray(body, body + size);
    if (idChunk === 'data') data = buf.subarray(body, body + size);
    pos = body + size + (size & 1);
  }
  if (!fmtChunk || !data) throw new Error(`${path}: missing fmt/data chunk`);
  let tag = fmtChunk.readUInt16LE(0);
  const channels = fmtChunk.readUInt16LE(2);
  const rate = fmtChunk.readUInt32LE(4);
  const bits = fmtChunk.readUInt16LE(14);
  if (tag === 0xfffe) tag = fmtChunk.readUInt16LE(24);
  const bytes = bits / 8;
  const frames = Math.floor(data.length / (bytes * channels));
  const mono = new Float64Array(frames);
  for (let f = 0; f < frames; f++) {
    let sum = 0;
    for (let c = 0; c < channels; c++) {
      const o = (f * channels + c) * bytes;
      let v;
      if (tag === 3 && bits === 32) v = data.readFloatLE(o);
      else if (bits === 16) v = data.readInt16LE(o) / 32768;
      else if (bits === 24) v = data.readIntLE(o, 3) / 8388608;
      else if (bits === 32) v = data.readInt32LE(o) / 2147483648;
      else throw new Error(`${path}: unsupported ${bits}-bit format ${tag}`);
      sum += v * v;
    }
    // Power-mono: the channel-average power, so a level change shows the
    // same in mono and stereo recordings.
    mono[f] = Math.sqrt(sum / channels);
  }
  return { rate, channels, mono };
}

function levels({ mono }) {
  let sum = 0;
  let peak = 0;
  for (const v of mono) {
    sum += v * v;
    peak = Math.max(peak, v);
  }
  return { rmsDb: 10 * Math.log10(sum / Math.max(1, mono.length)), peakDb: 20 * Math.log10(peak) };
}

/** 10 ms power envelope in dB. */
function envelope({ rate, mono }) {
  const hop = Math.round(rate / 100);
  const out = [];
  for (let i = 0; i + hop <= mono.length; i += hop) {
    let sum = 0;
    for (let j = i; j < i + hop; j++) sum += mono[j] * mono[j];
    out.push(10 * Math.log10(sum / hop + 1e-20));
  }
  return out;
}

/**
 * Level of `test` against `ref` in dB. Aligns the two envelopes by
 * maximising their correlation over +-maxLagFrames (a level offset does not
 * move the best lag), then takes the median per-frame difference over frames
 * where both are within 40 dB of their loudest.
 */
export function compareEnvelopes(ref, test, maxLagFrames = 500) {
  const floor = (env) => Math.max(...env) - 40;
  const fr = floor(ref);
  const ft = floor(test);
  let best = { lag: 0, score: -Infinity };
  for (let lag = -maxLagFrames; lag <= maxLagFrames; lag++) {
    let n = 0;
    let sa = 0, sb = 0, sab = 0, saa = 0, sbb = 0;
    for (let i = Math.max(0, -lag); i < ref.length && i + lag < test.length; i++) {
      const a = Math.max(ref[i], fr - 20);
      const b = Math.max(test[i + lag], ft - 20);
      sa += a; sb += b; sab += a * b; saa += a * a; sbb += b * b; n++;
    }
    if (n < 100) continue;
    const cov = sab - (sa * sb) / n;
    const den = Math.sqrt((saa - (sa * sa) / n) * (sbb - (sb * sb) / n));
    const score = den > 0 ? cov / den : -Infinity;
    if (score > best.score) best = { lag, score };
  }
  const diffs = [];
  for (let i = Math.max(0, -best.lag); i < ref.length && i + best.lag < test.length; i++) {
    if (ref[i] > fr && test[i + best.lag] > ft) diffs.push(test[i + best.lag] - ref[i]);
  }
  diffs.sort((x, y) => x - y);
  const q = (p) => diffs[Math.min(diffs.length - 1, Math.floor(p * diffs.length))];
  return { lagFrames: best.lag, correlation: best.score, frames: diffs.length, median: q(0.5), p25: q(0.25), p75: q(0.75) };
}

function compare(opts) {
  const [a, b] = opts._;
  if (!a || !b) throw new Error('usage: compare reference.wav test.wav');
  const ra = readWav(a);
  const rb = readWav(b);
  const la = levels(ra);
  const lb = levels(rb);
  console.log(`${a}: rms ${fmt(la.rmsDb).trim()} FS, peak ${fmt(la.peakDb).trim()} FS`);
  console.log(`${b}: rms ${fmt(lb.rmsDb).trim()} FS, peak ${fmt(lb.peakDb).trim()} FS`);
  if (Math.max(la.peakDb, lb.peakDb) > -0.5) console.log('warning: a recording reaches full scale (clipped input); lower the input gain');
  const r = compareEnvelopes(envelope(ra), envelope(rb));
  if (!r.frames) throw new Error('no overlapping audio to compare');
  console.log(`aligned  offset ${(r.lagFrames * 10).toFixed(0)} ms, envelope correlation ${r.correlation.toFixed(3)}, ${r.frames} frames`);
  console.log(`level    ${b} is ${fmt(r.median).trim()} vs ${a} (middle half ${r.p25.toFixed(2)}..${r.p75.toFixed(2)} dB)`);
  if (r.correlation < 0.8) console.log('warning: low correlation; the recordings may not cover the same passage');
}

const commands = { snapshot, record, compare };

if (import.meta.url === `file://${process.argv[1]}`) {
  const opts = args(process.argv.slice(2));
  const command = commands[opts._.shift()];
  if (!command) {
    console.error('usage: level-probe.mjs snapshot|record|compare (see the header of this file)');
    process.exit(2);
  }
  try {
    command(opts);
  } catch (e) {
    console.error(e.message);
    process.exit(1);
  }
}
