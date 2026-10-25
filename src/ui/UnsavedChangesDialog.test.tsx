import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { UnsavedChangesDialog } from "./UnsavedChangesDialog";

describe("UnsavedChangesDialog", () => {
  it("offers Save, Discard, and Cancel before closing a dirty document", () => {
    const markup = renderToStaticMarkup(
      <UnsavedChangesDialog closing disabled={false} onChoice={() => {}} />,
    );

    expect(markup).toContain('role="alertdialog"');
    expect(markup).toContain('aria-modal="true"');
    expect(markup).toContain("Save your changes before closing the window?");
    expect(markup).toContain(">Save</button>");
    expect(markup).toContain(">Discard</button>");
    expect(markup).toContain(">Cancel</button>");
  });
});
