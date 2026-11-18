import { readdirSync } from "node:fs";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import catalogFixture from "../../../tests/golden/stage-1-catalog.json";
import type { ComponentCatalog } from "../../generated/ComponentCatalog";
import { symbols, type SymbolDefinition } from "../../symbols";
import {
  PartPreview,
  placeholderInitials,
  previewViewBox,
  symbolBounds,
} from "./PartPreview";

const catalog = catalogFixture as ComponentCatalog;

describe("symbol bounds", () => {
  it("covers every polyline point and the whole of every circle", () => {
    expect(
      symbolBounds([
        {
          kind: "polyline",
          points: [
            { x: -1, y: 0 },
            { x: 2, y: 1 },
          ],
        },
        { kind: "circle", center: { x: 0, y: -1 }, radius: 0.5 },
      ]),
    ).toEqual({ minX: -1, minY: -1.5, maxX: 2, maxY: 1 });
  });

  it("is null for an empty drawing", () => {
    expect(symbolBounds([])).toBeNull();
  });

  it("gives a view box in the preview's aspect ratio around the drawing", () => {
    const [x, y, width, height] = previewViewBox({
      minX: -1,
      minY: -0.25,
      maxX: 1,
      maxY: 0.25,
    })
      .split(" ")
      .map(Number);

    expect(width! / height!).toBeCloseTo(44 / 28, 2);
    expect(x! + width! / 2).toBeCloseTo(0, 2);
    expect(y! + height! / 2).toBeCloseTo(0, 2);
    // The drawing fits inside with a margin.
    expect(x!).toBeLessThan(-1);
    expect(x! + width!).toBeGreaterThan(1);
  });
});

describe("the part preview", () => {
  it("draws every symbol of the registry from its resting primitives", () => {
    for (const [typeId, symbol] of Object.entries(symbols)) {
      const markup = renderToStaticMarkup(
        <PartPreview symbol={symbol} name={typeId} />,
      );
      const drawn =
        (markup.match(/<polyline/g)?.length ?? 0) +
        (markup.match(/<circle/g)?.length ?? 0);

      expect(markup, typeId).toContain("<svg");
      expect(drawn, typeId).toBe(symbol!.primitives.length);
    }
  });

  it("equals the symbol's own points, so it can never differ from the canvas", () => {
    const symbol = symbols.resistor!;
    const markup = renderToStaticMarkup(
      <PartPreview symbol={symbol} name="Resistor" />,
    );

    expect(markup).toContain('points="-1,0 -0.6,0"');
    expect(markup).toContain(
      'points="-0.6,-0.25 0.6,-0.25 0.6,0.25 -0.6,0.25 -0.6,-0.25"',
    );
  });

  it("follows the theme: it uses the current text color and no fixed color", () => {
    for (const symbol of Object.values(symbols)) {
      const markup = renderToStaticMarkup(
        <PartPreview symbol={symbol} name="x" />,
      );

      expect(markup).toContain('stroke="currentColor"');
      expect(markup).not.toMatch(/#[0-9a-fA-F]{3,8}|rgb\(|fill="[^n]/);
    }
  });

  it("is decorative: hidden from assistive technology and not focusable", () => {
    const markup = renderToStaticMarkup(
      <PartPreview symbol={symbols.diode} name="Diode" />,
    );

    expect(markup).toContain('aria-hidden="true"');
    expect(markup).toContain('focusable="false"');
  });
});

describe("the placeholder for a part without a symbol", () => {
  const missing: SymbolDefinition | undefined = undefined;

  it("shows the initials of the part's name", () => {
    const markup = renderToStaticMarkup(
      <PartPreview symbol={missing} name="Three-phase star source" />,
    );

    expect(markup).toContain("part-preview-placeholder");
    expect(markup).toContain(">TPS<");
    expect(markup).not.toContain("<svg");
    expect(markup).toContain('aria-hidden="true"');
  });

  it("builds up to three capital letters and never an empty label", () => {
    expect(placeholderInitials("DC voltage source")).toBe("DVS");
    expect(placeholderInitials("Wattmeter")).toBe("W");
    expect(placeholderInitials("AC voltmeter and more")).toBe("AVA");
    expect(placeholderInitials("  ")).toBe("?");
  });

  it("is used for a symbol whose drawing is empty", () => {
    const empty = { ...symbols.resistor!, primitives: [] };

    expect(
      renderToStaticMarkup(<PartPreview symbol={empty} name="Blank" />),
    ).toContain(">B<");
  });

  it("covers exactly the catalog parts that have no editor symbol", () => {
    const withoutSymbol = catalog.componentTypes
      .filter((entry) => !symbols[entry.typeId])
      .map((entry) => entry.typeId);

    expect(withoutSymbol.sort()).toEqual([
      "ac_ammeter",
      "ac_voltage_source",
      "ac_voltmeter",
      "three_phase_source_delta",
      "three_phase_source_star",
      "wattmeter",
    ]);
  });
});

describe("the part pictures of the owner", () => {
  it("are kept in docs/symbols under a catalog type ID", () => {
    const types = new Set(catalog.componentTypes.map((entry) => entry.typeId));
    const files = readdirSync(
      new URL("../../../docs/symbols/", import.meta.url),
    );
    const pictures = files.filter((name) => /\.(svg|png)$/.test(name));

    for (const name of pictures) {
      expect(types.has(name.replace(/\.(svg|png)$/, "") as never), name).toBe(
        true,
      );
    }
    expect(files).toContain("README.md");
  });
});
