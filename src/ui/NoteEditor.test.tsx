import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { SchematicDocumentStore } from "../store/document";
import { dividerSchematic } from "../store/fixtures";
import { NoteEditor, type NoteEditorProps } from "./NoteEditor";

function render(props: Partial<NoteEditorProps> = {}, text = "Main supply") {
  const store = new SchematicDocumentStore(
    dividerSchematic,
    () => "00000000-0000-4000-8000-0000000000a1",
  );
  const noteId = store.addNote({ x: 3, y: 4 }, text);

  return renderToStaticMarkup(
    <NoteEditor
      schematic={store.getState().schematic}
      noteId={noteId}
      position={{ left: 12, top: 34 }}
      enabled
      onCommit={() => null}
      onDelete={() => {}}
      onClose={() => {}}
      {...props}
    />,
  );
}

describe("the note editor", () => {
  it("is a non-modal dialog with one text area holding the note", () => {
    const html = render();

    expect(html).toContain('role="dialog"');
    expect(html).toContain('aria-modal="false"');
    expect(html).toContain("left:12px;top:34px");
    expect(html.match(/<textarea/g)).toHaveLength(1);
    expect(html).toContain(">Main supply</textarea>");
    expect(html).toContain("Note text");
  });

  it("keeps line breaks and any script in the text area", () => {
    const html = render({}, "Hauptschalter\nNotaus Главный");

    expect(html).toContain("Hauptschalter\nNotaus");
    expect(html).toContain("Главный");
  });

  it("is disabled while a project operation runs, and renders nothing for a gone note", () => {
    expect(render({ enabled: false })).toMatch(/<textarea[^>]*disabled=""/);
    expect(render({ noteId: "00000000-0000-4000-8000-0000000000ff" })).toBe("");
  });

  it("offers deleting the note", () => {
    expect(render()).toContain("Delete note");
  });
});
