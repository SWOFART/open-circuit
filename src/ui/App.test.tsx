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
    expect(markup).toContain(">New</button>");
    expect(markup).toContain(">Open</button>");
    expect(markup).toContain(">Save</button>");
    expect(markup).toContain(">Save As</button>");
    expect(markup).toContain(">Open Example</button>");
    expect(markup).toContain('aria-label="Component palette"');
    // Nothing to undo or redo in a freshly opened document.
    expect(markup).toMatch(/<button[^>]*disabled=""[^>]*>Undo<\/button>/);
    expect(markup).toMatch(/<button[^>]*disabled=""[^>]*>Redo<\/button>/);
    expect(markup).toContain("Inspector");
    expect(markup).toContain("Results");
  });

  it("offers the assignment authoring panel, closed until it is asked for", () => {
    const markup = renderToStaticMarkup(<App />);

    expect(markup).toMatch(
      /<button[^>]*aria-pressed="false"[^>]*>Author assignment<\/button>/,
    );
    expect(markup).not.toContain('aria-label="Assignment authoring"');
    expect(markup).toContain('class="workspace-body"');
  });

  it("offers the student's assignment panel, closed until it is asked for", () => {
    const markup = renderToStaticMarkup(<App />);

    expect(markup).toMatch(
      /<button[^>]*aria-pressed="false"[^>]*>Student assignment<\/button>/,
    );
    expect(markup).not.toContain('aria-label="Student assignment"');
    // Without an assignment nothing is locked and the palette is not limited.
    expect(markup).not.toContain("locked by the assignment");
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
