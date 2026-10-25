import type { NumericDomain } from "../generated/NumericDomain";
import type { ParameterDefinition } from "../generated/ParameterDefinition";
import type { Unit } from "../generated/Unit";

// Unit prefixes are a presentation concern: values are converted to plain SI
// numbers before they cross the UI/Engine boundary.

export const unitSymbols: Readonly<Record<Unit, string>> = {
  ohm: "Ω",
  volt: "V",
  amp: "A",
  farad: "F",
  henry: "H",
};

const unitWords: Readonly<Record<Unit, readonly string[]>> = {
  ohm: ["Ω", "ohm", "ohms"],
  volt: ["v", "volt", "volts"],
  amp: ["a", "amp", "amps"],
  farad: ["f", "farad", "farads"],
  henry: ["h", "henry", "henries"],
};

const prefixes = [
  { symbol: "G", factor: 1e9 },
  { symbol: "M", factor: 1e6 },
  { symbol: "k", factor: 1e3 },
  { symbol: "", factor: 1 },
  { symbol: "m", factor: 1e-3 },
  { symbol: "µ", factor: 1e-6 },
  { symbol: "n", factor: 1e-9 },
  { symbol: "p", factor: 1e-12 },
] as const;

// Prefix letters accepted when parsing, as powers of ten. Case matters: `m` is
// milli and `M` is mega. Powers of ten keep results exact: `4.7k` is parsed as
// the decimal 4.7e3, which is exactly 4700, rather than 4.7 * 1000.
const parsePrefixExponents: Readonly<Record<string, number>> = {
  G: 9,
  M: 6,
  k: 3,
  K: 3,
  m: -3,
  µ: -6,
  μ: -6,
  u: -6,
  n: -9,
  p: -12,
};

const NUMBER_PATTERN =
  /^([+-]?(?:\d+(?:[.,]\d*)?|[.,]\d+))(?:[eE]([+-]?\d+))?\s*(.*)$/;

/**
 * Every reading and input is shown with this many significant digits
 * (docs/contracts/stage-1.md section 6). Seven digits keep the displayed Stage 1
 * fixture values within the acceptance tolerances of 1 µV and 1 nA of the exact
 * ones. Rounding is for display only; the unrounded number is what is sent to the
 * Engine and what tolerances are checked against.
 */
export const DISPLAY_SIGNIFICANT_DIGITS = 7;

/** Formats an SI value with a prefix, such as `1 kΩ` or `2.499875 V`. */
export function formatSi(value: number, unit: Unit): string {
  const symbol = unitSymbols[unit];

  if (!Number.isFinite(value)) {
    return "—";
  }

  if (value === 0) {
    return `0 ${symbol}`;
  }

  const magnitude = Math.abs(value);
  const prefix =
    prefixes.find((candidate) => magnitude >= candidate.factor) ??
    prefixes[prefixes.length - 1];
  const mantissa = Number(
    (value / prefix.factor).toPrecision(DISPLAY_SIGNIFICANT_DIGITS),
  );

  return `${mantissa} ${prefix.symbol}${symbol}`;
}

export type ParseFailure = "empty" | "format" | "not-finite";

export type ParseResult =
  | { readonly ok: true; readonly value: number }
  | { readonly ok: false; readonly reason: ParseFailure };

function isUnitWord(text: string, unit: Unit): boolean {
  // Lower-case both sides: `Ω` lower-cases to `ω`, and the ohm sign U+2126
  // lower-cases to the same letter, so both spellings are accepted.
  const lowered = text.toLowerCase();

  return unitWords[unit].some((word) => word.toLowerCase() === lowered);
}

/**
 * Parses text such as `1k`, `2.2 MΩ`, or `470 ohm` into a plain SI number.
 * It does not check the parameter's domain; see `evaluateParameterDraft`.
 */
export function parseSi(text: string, unit: Unit): ParseResult {
  const trimmed = text.trim();

  if (trimmed === "") {
    return { ok: false, reason: "empty" };
  }

  const match = NUMBER_PATTERN.exec(trimmed);

  if (!match) {
    return { ok: false, reason: "format" };
  }

  const [, mantissaText = "", exponentText = "0", suffix = ""] = match;

  if (mantissaText.includes(",") && mantissaText.includes(".")) {
    return { ok: false, reason: "format" };
  }

  let prefixExponent: number | undefined;

  if (suffix === "" || isUnitWord(suffix, unit)) {
    prefixExponent = 0;
  } else {
    const candidate = parsePrefixExponents[suffix.charAt(0)];
    const remainder = suffix.slice(1).trim();

    if (
      candidate !== undefined &&
      (remainder === "" || isUnitWord(remainder, unit))
    ) {
      prefixExponent = candidate;
    }
  }

  if (prefixExponent === undefined) {
    return { ok: false, reason: "format" };
  }

  const exponent = Number(exponentText) + prefixExponent;
  const value = Number(`${mantissaText.replace(",", ".")}e${exponent}`);

  return Number.isFinite(value)
    ? { ok: true, value }
    : { ok: false, reason: "not-finite" };
}

/** Mirrors the Rust `NumericDomain::accepts`; the Engine still validates. */
export function domainAccepts(domain: NumericDomain, value: number): boolean {
  if (!Number.isFinite(value)) {
    return false;
  }

  switch (domain) {
    case "finite":
      return true;
    case "strictly_positive":
      return value > 0;
    case "non_negative":
      return value >= 0;
  }
}

export type DraftEvaluation =
  | { readonly state: "invalid"; readonly reason: ParseFailure | "domain" }
  | { readonly state: "unchanged" }
  | { readonly state: "commit"; readonly value: number };

/**
 * Decides what a typed value means: an invalid field is never sent as valid,
 * and a value equal to the committed one needs no request.
 */
export function evaluateParameterDraft(
  text: string,
  definition: Pick<ParameterDefinition, "unit" | "domain">,
  committedValue: number,
): DraftEvaluation {
  // A value is displayed rounded (`formatSi`), so showing it back unedited must
  // not count as a change: re-parsing the rounded text would silently alter it.
  if (text.trim() === formatSi(committedValue, definition.unit)) {
    return { state: "unchanged" };
  }

  const parsed = parseSi(text, definition.unit);

  if (!parsed.ok) {
    return { state: "invalid", reason: parsed.reason };
  }

  if (!domainAccepts(definition.domain, parsed.value)) {
    return { state: "invalid", reason: "domain" };
  }

  return parsed.value === committedValue
    ? { state: "unchanged" }
    : { state: "commit", value: parsed.value };
}
