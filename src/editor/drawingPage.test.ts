import { describe, expect, it } from "vitest";
import type { TitleBlock } from "../generated/TitleBlock";
import { drawingSvg } from "./drawingExport";
import { drawingPage, fittingPageSize, pageDimensions } from "./drawingPage";
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
  it("uses millimetres and real escaped SVG text deterministically", () => {
    const page = drawingPage(scene, title);
    expect(page.marks).toContainEqual({
      kind: "polyline",
      points: [
        { x: 11, y: 20 },
        { x: 61, y: 20 },
      ],
    });
    const svg = drawingSvg(page);
    expect(svg).toBe(drawingSvg(drawingPage(scene, title)));
    expect(svg).toContain('width="297mm" height="210mm"');
    expect(svg).toContain('font-family="sans-serif"');
    expect(svg).toContain("Über &amp; Щит &lt;A&gt;</text>");
    expect(svg).toContain(">1 of 1</text>");
    expect(svg).toContain('cx="36" cy="20" r="0.6" fill="black"');
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
