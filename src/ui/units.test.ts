import { describe, expect, it } from "vitest";
import {
  domainAccepts,
  evaluateParameterDraft,
  formatSi,
  parseSi,
} from "./units";

describe("parseSi", () => {
  it.each([
    ["1000", "ohm", 1000],
    ["470", "ohm", 470],
    ["1k", "ohm", 1000],
    ["1 kΩ", "ohm", 1000],
    ["4.7k", "ohm", 4700],
    ["2.2M", "ohm", 2200000],
    ["2,2M", "ohm", 2200000],
    ["470 ohm", "ohm", 470],
    ["100m", "ohm", 0.1],
    ["100 mΩ", "ohm", 0.1],
    ["1u", "volt", 0.000001],
    ["3.3 V", "volt", 3.3],
    ["1e3", "ohm", 1000],
    ["-5", "volt", -5],
    ["10 MOhm", "ohm", 10000000],
    [".5k", "ohm", 500],
  ] as const)("reads %j as %j SI units", (text, unit, expected) => {
    expect(parseSi(text, unit)).toEqual({ ok: true, value: expected });
  });

  it("gives an exact number rather than a rounded product", () => {
    // 4.7 * 1000 is not exactly 4700 in binary floating point.
    expect(parseSi("4.7k", "ohm")).toEqual({ ok: true, value: 4700 });
    expect(parseSi("0.3k", "ohm")).toEqual({ ok: true, value: 300 });
  });

  it("treats m as milli and M as mega", () => {
    expect(parseSi("5m", "ohm")).toEqual({ ok: true, value: 0.005 });
    expect(parseSi("5M", "ohm")).toEqual({ ok: true, value: 5000000 });
  });

  it.each([
    ["", "ohm", "empty"],
    ["   ", "ohm", "empty"],
    ["abc", "ohm", "format"],
    ["1x", "ohm", "format"],
    ["1 kV", "ohm", "format"],
    ["1 kΩ", "volt", "format"],
    ["1,000.5", "ohm", "format"],
    ["1e999", "ohm", "not-finite"],
    ["9e300 G", "ohm", "not-finite"],
  ] as const)("rejects %j as %s", (text, unit, reason) => {
    expect(parseSi(text, unit)).toEqual({ ok: false, reason });
  });
});

describe("formatSi", () => {
  it.each([
    [1000, "ohm", "1 kΩ"],
    [4700, "ohm", "4.7 kΩ"],
    [0.1, "ohm", "100 mΩ"],
    [0, "ohm", "0 Ω"],
    [2.4998750062496877, "volt", "2.49988 V"],
    [3.749906252343691, "volt", "3.74991 V"],
    [-0.0025, "amp", "-2.5 mA"],
    [0.000001, "volt", "1 µV"],
    [1.5e12, "ohm", "1500 GΩ"],
  ] as const)("formats %j %s as %j", (value, unit, expected) => {
    expect(formatSi(value, unit)).toBe(expected);
  });

  it("never prints NaN or Infinity as a reading", () => {
    expect(formatSi(Number.NaN, "volt")).toBe("—");
    expect(formatSi(Number.POSITIVE_INFINITY, "volt")).toBe("—");
    expect(formatSi(Number.NEGATIVE_INFINITY, "ohm")).toBe("—");
  });

  it("round-trips a displayed value to the same number", () => {
    for (const value of [1000, 4700, 0.1, 2200000, 0.000047]) {
      const text = formatSi(value, "ohm");

      expect(parseSi(text, "ohm")).toEqual({ ok: true, value });
    }
  });
});

describe("domainAccepts", () => {
  it.each([
    ["finite", -5, true],
    ["finite", 0, true],
    ["finite", Number.NaN, false],
    ["finite", Number.POSITIVE_INFINITY, false],
    ["strictly_positive", 1, true],
    ["strictly_positive", 0, false],
    ["strictly_positive", -1, false],
    ["non_negative", 0, true],
    ["non_negative", -0.001, false],
    ["non_negative", Number.NaN, false],
  ] as const)("%s accepts %j: %j", (domain, value, expected) => {
    expect(domainAccepts(domain, value)).toBe(expected);
  });
});

describe("evaluateParameterDraft", () => {
  const resistance = { unit: "ohm", domain: "strictly_positive" } as const;

  it("asks to commit a changed, valid value in SI units", () => {
    expect(evaluateParameterDraft("3k", resistance, 1000)).toEqual({
      state: "commit",
      value: 3000,
    });
  });

  it("sends nothing when the value did not change", () => {
    expect(evaluateParameterDraft("1 kΩ", resistance, 1000)).toEqual({
      state: "unchanged",
    });
  });

  it("does not treat an unedited, rounded display as a change", () => {
    // 1234.5678 is shown as "1.23457 kΩ"; blurring the field without editing it
    // must not re-parse that rounded text and silently change the value.
    const committed = 1234.5678;
    const shown = formatSi(committed, "ohm");

    expect(shown).toBe("1.23457 kΩ");
    expect(evaluateParameterDraft(shown, resistance, committed)).toEqual({
      state: "unchanged",
    });
    expect(evaluateParameterDraft(` ${shown} `, resistance, committed)).toEqual(
      {
        state: "unchanged",
      },
    );
    expect(evaluateParameterDraft("1234.5679", resistance, committed)).toEqual({
      state: "commit",
      value: 1234.5679,
    });
  });

  it.each([
    ["", "empty"],
    ["abc", "format"],
    ["0", "domain"],
    ["-5", "domain"],
    ["1e999", "not-finite"],
  ] as const)("marks %j invalid (%s) so it is never sent", (text, reason) => {
    expect(evaluateParameterDraft(text, resistance, 1000)).toEqual({
      state: "invalid",
      reason,
    });
  });

  it("allows zero for a non-negative parameter and negatives for a finite one", () => {
    expect(
      evaluateParameterDraft("0", { unit: "ohm", domain: "non_negative" }, 0.1),
    ).toEqual({ state: "commit", value: 0 });
    expect(
      evaluateParameterDraft("-5", { unit: "volt", domain: "finite" }, 5),
    ).toEqual({ state: "commit", value: -5 });
  });
});
