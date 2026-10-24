import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

function readProjectFile(relativePath: string): string {
  return readFileSync(
    new URL(`../../${relativePath}`, import.meta.url),
    "utf8",
  );
}

// A Tauri window enforces its CSP; a browser or test run does not. Without
// these guards the canvas could pass every test and still fail to start in the
// desktop app with "Current environment does not allow unsafe-eval".
describe("canvas under the Tauri content security policy", () => {
  const config = JSON.parse(readProjectFile("crates/app/tauri.conf.json")) as {
    app: { security: { csp: string; devCsp: string } };
  };

  it("keeps the CSP free of unsafe-eval", () => {
    expect(config.app.security.csp).not.toContain("unsafe-eval");
    expect(config.app.security.devCsp).not.toContain("unsafe-eval");
  });

  it("makes PixiJS use its eval-free shader helpers", () => {
    const source = readProjectFile("src/editor/EditorCore.ts");
    const unsafeEvalImport = source.indexOf('import "pixi.js/unsafe-eval"');
    const pixiImport = source.indexOf('from "pixi.js"');

    expect(unsafeEvalImport).toBeGreaterThanOrEqual(0);
    expect(unsafeEvalImport).toBeLessThan(pixiImport);
  });
});
