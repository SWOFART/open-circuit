import { useEffect, useId, useRef, useState } from "react";
import { NOTE_MESSAGES, noteProblem } from "../editor/noteRules";
import type { Schematic } from "../generated/Schematic";
import type { StableId } from "../generated/StableId";

export interface NoteEditorProps {
  readonly schematic: Schematic;
  readonly noteId: StableId;
  /** Where the panel sits in its container, in pixels. */
  readonly position: { readonly left: number; readonly top: number };
  /** False while a project operation is pending. */
  readonly enabled: boolean;
  /** Stores the text and returns why it was refused, or null once it is stored. */
  readonly onCommit: (noteId: StableId, text: string) => string | null;
  readonly onDelete: (noteId: StableId) => void;
  /** Escape or the close button: the host returns the focus to the canvas. */
  readonly onClose: () => void;
}

/**
 * The text of one note, opened by a double-click or Enter on the canvas like a part's editor
 * (docs/contracts/stage-12-cabinet-documentation.md section 6): non-modal, one text area. Enter
 * adds a line break; Ctrl+Enter or leaving the field stores the text, Escape restores it, and a
 * refused text stays in the field with its reason.
 */
export function NoteEditor({
  schematic,
  noteId,
  position,
  enabled,
  onCommit,
  onDelete,
  onClose,
}: NoteEditorProps) {
  const note = schematic.documentation.notes.find(({ id }) => id === noteId);
  const inputId = useId();
  const messageId = `${inputId}-message`;
  const area = useRef<HTMLTextAreaElement>(null);
  const committed = note?.text ?? "";
  const [draft, setDraft] = useState(committed);
  const [refusal, setRefusal] = useState<string | null>(null);

  useEffect(() => {
    setDraft(committed);
    setRefusal(null);
  }, [committed]);

  // Opening moves the focus into the text. A double press opens the editor while the browser is
  // still handling the press, so the focus moves on the next frame.
  useEffect(() => {
    const frame = window.requestAnimationFrame(() => {
      area.current?.focus({ preventScroll: true });
      area.current?.select();
    });
    return () => window.cancelAnimationFrame(frame);
  }, [noteId]);

  if (!note) {
    return null;
  }

  const unchanged = draft === committed;
  const problem = unchanged ? null : noteProblem(draft);
  const reason = refusal ?? (problem === null ? null : NOTE_MESSAGES[problem]);

  function commit(): void {
    if (unchanged || problem !== null) return;
    setRefusal(onCommit(noteId, draft));
  }

  return (
    <section
      className="component-editor"
      role="dialog"
      aria-modal="false"
      aria-label="Edit note"
      style={position}
      onKeyDown={(event) => {
        if (event.key === "Escape") {
          event.preventDefault();
          event.stopPropagation();
          if (unchanged) {
            onClose();
          } else {
            // Restores the stored text; a second Escape closes the panel.
            setDraft(committed);
            setRefusal(null);
          }
        }
      }}
    >
      <div className="component-editor-header">
        <h2>Note</h2>
        <button
          type="button"
          className="toolbar-button"
          aria-label="Close the note editor"
          onClick={onClose}
        >
          Close
        </button>
      </div>
      <div className="parameter-field">
        <label htmlFor={inputId}>Note text</label>
        <textarea
          ref={area}
          id={inputId}
          rows={5}
          spellCheck
          value={draft}
          disabled={!enabled}
          aria-invalid={reason !== null}
          aria-describedby={reason !== null ? messageId : undefined}
          onChange={(event) => {
            setDraft(event.target.value);
            setRefusal(null);
          }}
          onBlur={commit}
          onKeyDown={(event) => {
            if (event.key === "Enter" && (event.ctrlKey || event.metaKey)) {
              event.preventDefault();
              commit();
            }
          }}
        />
        <p className="parameter-hint">
          Enter adds a line. Ctrl+Enter stores the text, Esc restores it.
        </p>
        {reason !== null && (
          <p id={messageId} className="field-error" role="alert">
            {reason}
          </p>
        )}
      </div>
      <button
        type="button"
        className="toolbar-button"
        disabled={!enabled}
        onClick={() => onDelete(noteId)}
      >
        Delete note
      </button>
    </section>
  );
}
