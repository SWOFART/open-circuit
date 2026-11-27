import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { SHORTCUT_GROUPS, SHORTCUTS, shortcutKeys } from "../editor/shortcuts";
import { message } from "./messages";
import { ShortcutWindow } from "./ShortcutWindow";

// The shortcut window of docs/contracts/stage-16-editor-improvements.md section 10, as markup:
// generated from the table the key handlers read.

describe("the shortcut window", () => {
  const render = (platform: "mac" | "other") =>
    renderToStaticMarkup(
      <ShortcutWindow platform={platform} onClose={() => {}} />,
    );

  it("is a labelled modal dialog with a close button", () => {
    const html = render("other");

    expect(html).toContain('role="dialog"');
    expect(html).toContain('aria-modal="true"');
    expect(html).toContain('aria-labelledby="shortcut-window-title"');
    expect(html).toContain("Keyboard shortcuts</h2>");
    expect(html).toContain(">Close<");
  });

  it("lists every binding of the table, by group, with the keys of the platform", () => {
    const other = render("other");
    const mac = render("mac");

    for (const group of SHORTCUT_GROUPS) {
      expect(other).toContain(`>${group}</h3>`);
    }
    for (const shortcut of SHORTCUTS) {
      const text = message(shortcut.description).replace(/&/g, "&amp;");
      expect(other, shortcut.id).toContain(text);
      for (const keys of shortcutKeys(shortcut, "other")) {
        expect(other, `${shortcut.id} ${keys}`).toContain(`<kbd>${keys}</kbd>`);
      }
      for (const keys of shortcutKeys(shortcut, "mac")) {
        expect(mac, `${shortcut.id} ${keys}`).toContain(`<kbd>${keys}</kbd>`);
      }
    }
    expect(other).toContain("<kbd>Ctrl+S</kbd>");
    expect(mac).toContain("<kbd>Cmd+S</kbd>");
    expect(mac).not.toContain("<kbd>Ctrl+S</kbd>");
  });
});
