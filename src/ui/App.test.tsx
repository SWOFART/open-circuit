import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { App } from "./App";

describe("application shell", () => {
  it("renders the circuit workspace and editor host", () => {
    const markup = renderToStaticMarkup(<App />);

    expect(markup).toContain("Circuit workspace");
    expect(markup).toContain('aria-label="Circuit editor"');
    expect(markup).toContain("editor-canvas");
    expect(markup).toContain("Rust catalog is available in the desktop app.");
    expect(markup).toContain("Simulation solver is not connected yet.");
  });
});
