import { describe, expect, it } from "vitest";
import type { TitleBlock } from "../generated/TitleBlock";
import { drawingSvg } from "./drawingExport";
import {
  drawingPage,
  fittingPageSize,
  pageDimensions,
  sheetDrawingPage,
} from "./drawingPage";
import { dividerSchematic } from "../store/fixtures";
import type { SceneModel } from "./scene";

const title: TitleBlock = {
  title: "Über & Щит <A>",
  drawingNumber: "R-173",
  revision: "B",
  author: "",
  company: "",
  date: "",
};

const scene: SceneModel = {
  bounds: { minX: -2, minY: 0, maxX: 8, maxY: 6 },
  shapes: [
    {
      kind: "polyline",
      role: "wire",
      owner: { kind: "wire", id: "wire" },
      points: [
        { x: -2, y: 2 },
        { x: 8, y: 2 },
      ],
    },
  ],
  connectionDots: [{ x: 3, y: 2 }],
};

describe("drawing pages", () => {
  it("clips a crossing conductor to each printable frame and numbers sheets in order", () => {
    const first = {
      id: "first",
      name: "Power",
      origin: { x: 0, y: 0 },
      size: "a4",
    } as const;
    const second = {
      id: "second",
      name: "Control",
      origin: { x: 59, y: 0 },
      size: "a3",
    } as const;
    const schematic = {
      ...dividerSchematic,
      documentation: {
        ...dividerSchematic.documentation,
        sheets: [first, second],
      },
    };
    const crossing: SceneModel = {
      bounds: { minX: 4, minY: 15, maxX: 80, maxY: 15 },
      shapes: [
        {
          kind: "polyline",
          role: "wire",
          owner: { kind: "wire", id: "wire" },
          points: [
            { x: 4, y: 15 },
            { x: 80, y: 15 },
          ],
        },
      ],
      connectionDots: [],
    };
    const a4 = drawingSvg(sheetDrawingPage(crossing, schematic, first, 0));
    const a3 = drawingSvg(sheetDrawingPage(crossing, schematic, second, 1));
    expect(a4).toContain('points="21,75 286,75"');
    expect(a3).toContain('points="10,76 105,76"');
    expect(a4).toContain(">1 of 2</text>");
    expect(a3).toContain(">2 of 2</text>");
  });

  it("identifies the explicit sheet when a wire enters its title block", () => {
    const sheet = {
      id: "power",
      name: "Power",
      origin: { x: 0, y: 0 },
      size: "a4",
    } as const;
    const schematic = {
      ...dividerSchematic,
      documentation: { ...dividerSchematic.documentation, sheets: [sheet] },
    };
    const crossing: SceneModel = {
      bounds: { minX: 4, minY: 35, maxX: 40, maxY: 35 },
      shapes: [
        {
          kind: "polyline",
          role: "wire",
          owner: { kind: "wire", id: "wire" },
          points: [
            { x: 4, y: 35 },
            { x: 40, y: 35 },
          ],
        },
      ],
      connectionDots: [],
    };
    expect(() => sheetDrawingPage(crossing, schematic, sheet, 0)).toThrow(
      "title block on sheet 1 (Power)",
    );
  });

  it("uses millimetres and real escaped SVG text deterministically", () => {
    const page = drawingPage(scene, title);
    expect(page.marks).toContainEqual({
      kind: "polyline",
      points: [
        { x: 13, y: 22 },
        { x: 63, y: 22 },
      ],
    });
    const svg = drawingSvg(page);
    expect(svg).toBe(drawingSvg(drawingPage(scene, title)));
    expect(svg).toContain('width="297mm" height="210mm"');
    expect(svg).toContain('font-family="sans-serif"');
    expect(svg).toContain("Über &amp; Щит &lt;A&gt;</text>");
    expect(svg).toContain(">1 of 1</text>");
    expect(svg).toContain('cx="38" cy="22" r="0.6" fill="black"');
  });

  it("picks A3 for larger drawings and refuses a drawing outside both sizes", () => {
    const wide = { ...scene.bounds, maxX: 60 };
    expect(fittingPageSize(wide)).toBe("a3");
    expect(() => drawingPage({ ...scene, bounds: wide }, title, "a4")).toThrow(
      "chosen page size",
    );
    expect(() => fittingPageSize({ ...wide, maxX: 100 })).toThrow("Add sheets");
    expect(pageDimensions.a4.pixels).toEqual([3508, 2480]);
    expect(pageDimensions.a3.pixels).toEqual([4961, 3508]);
  });

  it("keeps the drawing clear of the title block and wraps long title values", () => {
    const nearEdge: SceneModel = {
      bounds: { minX: 0, minY: 0, maxX: 40, maxY: 33 },
      shapes: [
        {
          kind: "polyline",
          role: "wire",
          owner: { kind: "wire", id: "wire" },
          points: [
            { x: 0, y: 0 },
            { x: 40, y: 0 },
            { x: 40, y: 33 },
          ],
        },
      ],
      connectionDots: [],
    };
    const longTitle = {
      ...title,
      title: "Щ".repeat(100),
      company: "W".repeat(64),
    };
    expect(() => drawingPage(nearEdge, longTitle, "a4")).toThrow("overlaps");
    const page = drawingPage(nearEdge, longTitle);
    expect(page.size).toBe("a3");
    const titleLines = page.marks.flatMap((mark) =>
      mark.kind === "text" && mark.text.startsWith("Щ") ? [mark.text] : [],
    );
    expect(titleLines.map((line) => line.length)).toEqual([52, 48]);
    expect(titleLines.join("")).toBe(longTitle.title);
  });
});
