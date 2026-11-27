/**
 * What the history panel shows for one history entry: a message key and its values, for example
 * `history.move` with a count (docs/contracts/stage-16-editor-improvements.md section 6). A label
 * is editor state: it is never saved, and an edit without one shows the generic "Edit".
 */
export interface HistoryLabel {
  readonly key: string;
  readonly values?: Readonly<Record<string, string | number>>;
}

/** The history keeps this many entries; the oldest is dropped first. */
export const HISTORY_LIMIT = 1000;
