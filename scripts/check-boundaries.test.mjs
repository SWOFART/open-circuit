import { describe, expect, it } from "vitest";
import { forbiddenCrateNames, importsReact } from "./check-boundaries.mjs";

describe("forbiddenCrateNames", () => {
  it("accepts a dependency tree without Tauri or UI crates", () => {
    const tree = [
      "open-circuit-engine v0.1.0 (D:\\open-circuit\\crates\\engine)",
      "serde v1.0.228",
      "ts-rs v12.0.1",
      "taurine v1.0.0",
    ].join("\n");

    expect(forbiddenCrateNames(tree)).toEqual([]);
  });

  it("reports Tauri, webview, and app crates once each", () => {
    const tree = [
      "open-circuit-engine v0.1.0 (/repo/crates/engine)",
      "tauri v2.12.5",
      "tauri-runtime v2.12.2",
      "wry v0.57.1",
      "tauri v2.12.5 (*)",
      "open-circuit-app v0.1.0 (/repo/crates/app)",
    ].join("\r\n");

    expect(forbiddenCrateNames(tree)).toEqual([
      "open-circuit-app",
      "tauri",
      "tauri-runtime",
      "wry",
    ]);
  });
});

describe("importsReact", () => {
  it.each([
    'import React from "react";',
    "import { useState } from 'react';",
    'import "react";',
    'import { createRoot } from "react-dom/client";',
    'export { useState } from "react";',
    'const react = await import("react");',
    'const react = require("react");',
  ])("detects %s", (source) => {
    expect(importsReact(source)).toBe(true);
  });

  it.each([
    'import { Application } from "pixi.js";',
    'import { thing } from "./react-helpers";',
    'import type { Schematic } from "../generated/Schematic";',
    "// React is only mentioned in a comment",
  ])("ignores %s", (source) => {
    expect(importsReact(source)).toBe(false);
  });
});
