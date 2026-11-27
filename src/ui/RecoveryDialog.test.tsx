import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import dividerFixture from "../../tests/golden/stage-1-divider.schematic.json";
import type { Schematic } from "../generated/Schematic";
import { formatRecoveryTime, RecoveryDialog } from "./RecoveryDialog";

// The offer at start (docs/contracts/stage-16-editor-improvements.md section 9), as markup.

const schematic = dividerFixture as Schematic;
const offer = {
  kind: "offer",
  writtenAtMs: Date.UTC(2026, 10, 27, 14, 30),
  sourcePath: "C:/work/lab.ocircuit",
  appVersion: "1.0.0",
  schematic,
} as const;

const render = (
  recovery: Parameters<typeof RecoveryDialog>[0]["recovery"],
  over: { disabled?: boolean; error?: string | null } = {},
) =>
  renderToStaticMarkup(
    <RecoveryDialog
      recovery={recovery}
      disabled={over.disabled ?? false}
      error={over.error ?? null}
      onRestore={() => {}}
      onDiscard={() => {}}
    />,
  );

describe("the recovery offer", () => {
  it("is a modal alert dialog with a title and a description", () => {
    const html = render(offer);

    expect(html).toContain('role="alertdialog"');
    expect(html).toContain('aria-modal="true"');
    expect(html).toContain('aria-labelledby="recovery-dialog-title"');
    expect(html).toContain('aria-describedby="recovery-dialog-description"');
  });

  it("names the project it came from and offers Restore and Discard", () => {
    const html = render(offer);

    expect(html).toContain("C:/work/lab.ocircuit");
    expect(html).toContain(">Restore<");
    expect(html).toContain(">Discard<");
    // The original project is not touched, and the restored document is a new one.
    expect(html).toContain("not touched");
  });

  it("says so for a document that was never saved", () => {
    expect(render({ ...offer, sourcePath: null })).toContain(
      "a document that was never saved",
    );
  });

  it("has no Escape or cancel choice: only the two answers", () => {
    const html = render(offer);

    expect(html.match(/<button/g)).toHaveLength(2);
    expect(html).not.toContain("Cancel");
  });

  it("offers an unreadable file for deletion with the reason, and no Restore", () => {
    const html = render({ kind: "unreadable", reason: "It is damaged." });

    expect(html).toContain("It is damaged.");
    expect(html).toContain(">Delete it<");
    expect(html).not.toContain(">Restore<");
    expect(html.match(/<button/g)).toHaveLength(1);
  });

  it("shows why an answer failed, as an alert, and disables the choices while one is working", () => {
    const html = render(offer, {
      error: "Could not delete it.",
      disabled: true,
    });

    expect(html).toContain('role="alert"');
    expect(html).toContain("Could not delete it.");
    expect(html.match(/disabled=""/g)).toHaveLength(2);
  });
});

describe("the time of the file", () => {
  it("reads as a date and a time in the zone asked for", () => {
    expect(
      formatRecoveryTime(Date.UTC(2026, 10, 27, 14, 30), "en-GB", "UTC"),
    ).toBe("27 Nov 2026, 14:30");
  });
});
