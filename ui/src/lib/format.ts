import { formatLocale } from "./locale.svelte";

/// Clock-style times (`4:36`, `10:16:08`) read the same in every supported
/// language; amounts, counts and dates below follow `formatLocale`.
export function formatDuration(seconds: number): string {
  const s = Math.max(0, Math.floor(seconds));
  const m = Math.floor(s / 60);
  const r = s % 60;
  return `${m}:${r.toString().padStart(2, "0")}`;
}

/// Total runtime for headers: minutes below an hour, otherwise hours.
export function formatTotalDuration(seconds: number): string {
  const s = Math.max(0, Math.floor(seconds));
  if (s < 3600) return formatDuration(s);
  const h = Math.floor(s / 3600);
  const m = Math.floor((s % 3600) / 60);
  const r = s % 60;
  return `${h}:${m.toString().padStart(2, "0")}:${r.toString().padStart(2, "0")}`;
}

/// Listening time for History and Stats: "45 s", "42 min", "3 h 05 min",
/// "2 d 4 h". Unlike formatTotalDuration this never reads as a clock time.
/// English keeps these short symbols; other languages use their own short
/// unit names (`Intl.NumberFormat` units: "3 Std. 05 Min.").
export function formatListenTime(ms: number): string {
  const s = Math.max(0, Math.floor(ms / 1000));
  const m = Math.floor(s / 60);
  const h = Math.floor(m / 60);
  const unit = units(formatLocale.value);
  if (s < 60) return unit.second(s);
  if (m < 60) return unit.minute(m);
  if (h < 48) return `${unit.hour(h)} ${unit.minute(m % 60, 2)}`;
  return `${unit.day(Math.floor(h / 24))} ${unit.hour(h % 24)}`;
}

type UnitFormat = (value: number, digits?: number) => string;
interface Units {
  second: UnitFormat;
  minute: UnitFormat;
  hour: UnitFormat;
  day: UnitFormat;
}

const ENGLISH_UNITS: Units = {
  second: (v) => `${v} s`,
  minute: (v, digits = 1) => `${String(v).padStart(digits, "0")} min`,
  hour: (v) => `${v} h`,
  day: (v) => `${v} d`,
};

const unitCache = new Map<string, Units>();

function units(locale: string): Units {
  if (locale === "en" || locale.startsWith("en-")) return ENGLISH_UNITS;
  let cached = unitCache.get(locale);
  if (!cached) {
    const make = (unit: string): UnitFormat => {
      const plain = new Intl.NumberFormat(locale, { style: "unit", unit, unitDisplay: "short" });
      const padded = new Intl.NumberFormat(locale, { style: "unit", unit, unitDisplay: "short", minimumIntegerDigits: 2 });
      return (value, digits = 1) => (digits > 1 ? padded : plain).format(value);
    };
    cached = { second: make("second"), minute: make("minute"), hour: make("hour"), day: make("day") };
    unitCache.set(locale, cached);
  }
  return cached;
}

/// Compact fan/listener counts in the UI language: 999, 9.7K, 12K, 1.1M, 1B
/// (German "9,7 Tsd.", Japanese "9672"/"1.2万"…).
export function formatCount(n: number): string {
  const value = Math.max(0, Math.floor(n));
  const parts = compactFormat(formatLocale.value).formatToParts(value);
  // Languages that do not abbreviate this size (German below a million)
  // get the exact number, not one rounded to three digits.
  if (!parts.some((part) => part.type === "compact")) return formatNumber(value);
  return parts.map((part) => part.value).join("");
}

const compactCache = new Map<string, Intl.NumberFormat>();

function compactFormat(locale: string): Intl.NumberFormat {
  let format = compactCache.get(locale);
  if (!format) {
    // One decimal below a hundred, none above ("9.7K", "999M"), whichever
    // is coarser.
    // (`roundingPriority` is ES2023; the TypeScript lib here predates it.)
    const options: Intl.NumberFormatOptions & { roundingPriority: "lessPrecision" } = {
      notation: "compact",
      maximumFractionDigits: 1,
      maximumSignificantDigits: 3,
      roundingPriority: "lessPrecision",
    };
    format = new Intl.NumberFormat(locale, options);
    compactCache.set(locale, format);
  }
  return format;
}

/// A plain number with the UI language's grouping and decimal marks.
export function formatNumber(n: number, fractionDigits = 0): string {
  return new Intl.NumberFormat(formatLocale.value, {
    minimumFractionDigits: fractionDigits,
    maximumFractionDigits: fractionDigits,
  }).format(n);
}

/// A whole percentage (`120` → "120%", German "120 %", Turkish "%120").
export function formatPercent(percent: number): string {
  return new Intl.NumberFormat(formatLocale.value, { style: "percent" }).format(percent / 100);
}

/// Engine notices are lowercase clauses ("exclusive mode unavailable; …") so
/// Rust can compare and compose them; shown on their own they start a
/// sentence. Only the first letter changes.
export function sentenceCase(text: string): string {
  return text.charAt(0).toLocaleUpperCase(formatLocale.value) + text.slice(1);
}

/// TIDAL release dates (`2014-02-11`, sometimes just a year) as `Feb 11, 2014`
/// in the UI language. The date is a calendar day, formatted in UTC so no
/// time zone shifts it. Anything unparseable yields "" so callers can omit it.
export function formatReleaseDate(date: string | null | undefined): string {
  const match = /^(\d{4})(?:-(\d{2})-(\d{2}))?/.exec(date ?? "");
  if (!match) return "";
  const [, year, month, day] = match;
  const m = Number(month);
  if (!month || !day || m < 1 || m > 12) return year;
  return new Intl.DateTimeFormat(formatLocale.value, {
    year: "numeric",
    month: "short",
    day: "numeric",
    timeZone: "UTC",
  }).format(Date.UTC(Number(year), m - 1, Number(day)));
}
