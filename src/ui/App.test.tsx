import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { App } from "./App";

describe("application shell", () => {
  it("renders the circuit workspace, editor host, and side panels", () => {
    const markup = renderToStaticMarkup(<App />);

    expect(markup).toContain("Circuit workspace");
    expect(markup).toContain('aria-label="Circuit editor"');
    expect(markup).toContain("editor-canvas");
    expect(markup).toContain("Fit to circuit");
    expect(markup).toContain("Inspector");
    expect(markup).toContain("Results");
  });

  it("waits for the backend before showing any reading", () => {
    const markup = renderToStaticMarkup(<App />);

    expect(markup).toContain("Waiting for the component catalog.");
    expect(markup).toContain("Waiting for the simulation backend");
    expect(markup).not.toContain("NaN");
    expect(markup).not.toContain("2.499875");
  });

  it("does not claim mock data unless a development mock was requested", () => {
    const markup = renderToStaticMarkup(<App />);

    expect(markup).toContain("Values come from the Rust engine only.");
    expect(markup).not.toContain("Development mock");
  });
});
