/**
 * The text rules of a note (docs/contracts/stage-12-cabinet-documentation.md section 6),
 * mirrored for an immediate check. The Engine's validation is the authority.
 */

export const NOTE_MAX_CHARACTERS = 500;
export const NOTE_MAX_LINES = 10;
export const MAX_NOTES = 200;
/** What a note placed with the Note tool says until the user edits it. */
export const DEFAULT_NOTE_TEXT = "Note";

export type NoteProblem = "empty" | "tooLong" | "tooManyLines" | "control";

// Every control character (U+0000 to U+001F, U+007F to U+009F) except the line feed.
// eslint-disable-next-line no-control-regex
const CONTROL = /[\u0000-\u0009\u000b-\u001f\u007f-\u009f]/;

/** Why `text` cannot be a note, or null. Line breaks are allowed, other control characters not. */
export function noteProblem(text: string): NoteProblem | null {
  const characters = [...text].length;
  if (characters === 0) return "empty";
  if (characters > NOTE_MAX_CHARACTERS) return "tooLong";
  if (CONTROL.test(text)) return "control";
  // Counted as the Engine does: one final line break does not start another line.
  const lines = text.split("\n").length - (text.endsWith("\n") ? 1 : 0);
  if (lines > NOTE_MAX_LINES) return "tooManyLines";
  return null;
}

export const NOTE_MESSAGES: Readonly<Record<NoteProblem, string>> = {
  empty: "A note cannot be empty. Delete the note instead.",
  tooLong: `A note is at most ${NOTE_MAX_CHARACTERS} characters.`,
  tooManyLines: `A note is at most ${NOTE_MAX_LINES} lines.`,
  control:
    "A note holds plain text: no control characters other than line breaks.",
};

/** The width and height of a note's text in grid units, estimated from its lines. */
export function noteExtent(text: string): {
  readonly width: number;
  readonly height: number;
} {
  const lines = text.split("\n");
  const longest = Math.max(...lines.map((line) => [...line].length));

  return {
    width: Math.max(1, longest * 0.2),
    height: Math.max(0.6, lines.length * 0.5),
  };
}
