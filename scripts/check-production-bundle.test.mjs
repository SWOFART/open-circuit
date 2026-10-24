import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
import {
  findMockMarkers,
  missingRequiredMarkers,
  mockMarkers,
  requiredMarkers,
} from "./check-production-bundle.mjs";

function source(path) {
  return readFileSync(new URL(`../${path}`, import.meta.url), "utf8");
}

describe("findMockMarkers", () => {
  it("accepts a bundle without the mock", () => {
    expect(
      findMockMarkers("The Rust engine is only available in the desktop app."),
    ).toEqual([]);
  });

  it.each(mockMarkers)("finds %j", (marker) => {
    expect(findMockMarkers(`before ${marker} after`)).toEqual([marker]);
  });

  it("reports every marker present", () => {
    expect(findMockMarkers(mockMarkers.join("\n"))).toEqual(mockMarkers);
  });
});

describe("missingRequiredMarkers", () => {
  it("passes a bundle that holds the production adapter", () => {
    expect(missingRequiredMarkers(requiredMarkers.join("\n"))).toEqual([]);
  });

  it("flags a bundle that does not look like the application", () => {
    expect(missingRequiredMarkers("unrelated code")).toEqual(requiredMarkers);
  });
});

// The checker only protects anything while its strings still match the code. If the mock
// or the production adapter is reworded, this fails and the markers must be updated.
describe("the markers match the application source", () => {
  const mockSource = [
    source("src/store/mockEngineAdapter.ts"),
    source("src/ui/App.tsx"),
  ].join("\n");

  it.each(mockMarkers)("the mock still contains %j", (marker) => {
    expect(mockSource).toContain(marker);
  });

  it.each(requiredMarkers)(
    "the production adapter still contains %j",
    (marker) => {
      expect(source("src/store/engineAdapter.ts")).toContain(marker);
    },
  );
});
