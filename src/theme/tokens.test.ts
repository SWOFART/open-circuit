import { describe, expect, it } from "vitest";
import { contrastRatio } from "./contrast";
import {
  colorNumber,
  cssVariable,
  THEMES,
  TOKEN_NAMES,
  traceVariable,
  type ThemeName,
  type TokenName,
} from "./tokens";

// The contrast rules of docs/contracts/stage-16-editor-improvements.md section 8, computed from
// the tokens: text against its surface at least 4.5:1, and wires, symbols, borders and focus
// outlines against their surface at least 3:1, in both themes. A token that breaks one fails the
// build.

const THEME_NAMES: readonly ThemeName[] = ["light", "dark"];
const TEXT_RATIO = 4.5;
const GRAPHIC_RATIO = 3;

/** The surfaces the interface text sits on. */
const SURFACES: readonly TokenName[] = [
  "surface-page",
  "surface-panel",
  "surface-raised",
  "surface-inset",
  "surface-hover",
  "surface-selected",
];
const TEXTS: readonly TokenName[] = [
  "text",
  "text-soft",
  "text-muted",
  "text-accent",
  "text-warning",
  "text-warning-soft",
  "text-warning-strong",
  "text-danger",
];
/** Pairs where a text token sits on one particular colored surface. */
const TEXT_ON: readonly (readonly [TokenName, TokenName])[] = [
  ["text-on-accent", "surface-accent"],
  ["text-on-accent", "surface-warning"],
  ["text-danger-strong", "surface-danger"],
  ["text-danger-strong", "surface-danger-faint"],
  ["text-danger", "surface-danger-faint"],
  ["text-success", "surface-success"],
  ["canvas-label", "canvas-background"],
  ["canvas-selected-label", "canvas-background"],
];
/** Lines that outline a control, a state or a focused element. */
const LINES: readonly TokenName[] = [
  "line-control",
  "line-dashed",
  "line-neutral",
  "line-focus",
  "line-accent",
  "line-warning",
  "line-warning-strong",
  "line-danger",
  "line-success",
];
const CANVAS_GRAPHICS: readonly TokenName[] = [
  "canvas-wire",
  "canvas-rail",
  "canvas-symbol",
  "canvas-reference",
  "canvas-junction",
  "canvas-selected",
  "canvas-detached",
  "canvas-cursor",
  "canvas-preview",
  "canvas-probe-first",
  "canvas-probe-second",
];
const TRACES = TOKEN_NAMES.filter((name) => name.startsWith("trace-"));

describe.each(THEME_NAMES)("the %s theme", (name) => {
  const tokens = THEMES[name];
  const ratio = (first: TokenName, second: TokenName) =>
    contrastRatio(tokens[first], tokens[second]);

  it("has a value for every token, in a form the renderers read", () => {
    for (const token of TOKEN_NAMES) {
      expect(tokens[token], token).toMatch(
        token === "shadow" || token === "scrim"
          ? /^rgb\(\d+ \d+ \d+ \/ \d+%\)$/
          : /^#[0-9a-f]{6}$/,
      );
    }
    expect(Object.keys(tokens).sort()).toEqual([...TOKEN_NAMES].sort());
  });

  it.each(TEXTS.flatMap((text) => SURFACES.map((surface) => [text, surface])))(
    "shows %s on %s at 4.5:1 or more",
    (text, surface) => {
      expect(
        ratio(text as TokenName, surface as TokenName),
      ).toBeGreaterThanOrEqual(TEXT_RATIO);
    },
  );

  it.each(TEXT_ON)("shows %s on %s at 4.5:1 or more", (text, surface) => {
    expect(ratio(text, surface)).toBeGreaterThanOrEqual(TEXT_RATIO);
  });

  it.each(
    LINES.flatMap((line) =>
      (
        [
          "surface-page",
          "surface-panel",
          "surface-raised",
          "surface-inset",
        ] as const
      ).map((surface) => [line, surface]),
    ),
  )("shows %s against %s at 3:1 or more", (line, surface) => {
    expect(
      ratio(line as TokenName, surface as TokenName),
    ).toBeGreaterThanOrEqual(GRAPHIC_RATIO);
  });

  it.each(CANVAS_GRAPHICS)(
    "shows %s against the canvas at 3:1 or more",
    (token) => {
      expect(ratio(token, "canvas-background")).toBeGreaterThanOrEqual(
        GRAPHIC_RATIO,
      );
    },
  );

  it.each(TRACES)(
    "shows %s against the plot surfaces at 3:1 or more",
    (trace) => {
      expect(ratio(trace, "surface-panel")).toBeGreaterThanOrEqual(
        GRAPHIC_RATIO,
      );
      expect(ratio(trace, "surface-inset")).toBeGreaterThanOrEqual(
        GRAPHIC_RATIO,
      );
    },
  );

  it("draws a grid that can be seen but is quieter than the wires", () => {
    expect(ratio("canvas-grid", "canvas-background")).toBeGreaterThan(1.1);
    expect(ratio("canvas-grid", "canvas-background")).toBeLessThan(
      ratio("canvas-wire", "canvas-background"),
    );
  });

  it("keeps the selection highlight apart from the wire", () => {
    expect(tokens["canvas-selected"]).not.toBe(tokens["canvas-wire"]);
    expect(tokens["canvas-detached"]).not.toBe(tokens["canvas-wire"]);
  });
});

describe("the two palettes", () => {
  it("differ, so a switch is a change", () => {
    expect(THEMES.light["surface-page"]).not.toBe(THEMES.dark["surface-page"]);
    expect(THEMES.light.text).not.toBe(THEMES.dark.text);
    expect(THEMES.light["canvas-background"]).not.toBe(
      THEMES.dark["canvas-background"],
    );
  });

  it("have a light page and a dark page", () => {
    expect(contrastRatio(THEMES.light["surface-page"], "#ffffff")).toBeLessThan(
      contrastRatio(THEMES.dark["surface-page"], "#ffffff"),
    );
  });
});

describe("how a token is handed on", () => {
  it("is a CSS custom property of the same name", () => {
    expect(cssVariable("surface-page")).toBe("--surface-page");
  });

  it("is a number for the canvas renderer", () => {
    expect(colorNumber("#60a5fa")).toBe(0x60a5fa);
    expect(() => colorNumber("rgb(0 0 0 / 45%)")).toThrow();
    expect(() => colorNumber("#fff")).toThrow();
  });

  it("gives each trace a variable and wraps round after the eighth", () => {
    expect(traceVariable(0)).toBe("var(--trace-1)");
    expect(traceVariable(7)).toBe("var(--trace-8)");
    expect(traceVariable(8)).toBe("var(--trace-1)");
  });
});
