import { HISTORY_LIMIT } from "../editor/historyLabel";
import type { HistorySummary } from "../store/document";
import { historyLabelText } from "./historyLabels";

export interface HistoryPanelProps {
  readonly history: HistorySummary;
  readonly disabled?: boolean;
  /** Goes to the state just after the entry at `index` (the past oldest first, then the undone). */
  readonly onGo: (index: number) => void;
}

/**
 * The history of the document, newest first (stage 16 contract, section 6). The entries after
 * the current point were undone: they are dimmed and marked, and choosing one redoes up to it.
 * Choosing an older entry undoes back to it, and the entry that is current does nothing. Each
 * move is made of ordinary undo and redo steps, so a keyboard user can do the same with Ctrl+Z.
 */
export function HistoryPanel({ history, disabled, onGo }: HistoryPanelProps) {
  const entries = [...history.past, ...history.future];
  const current = history.past.length - 1;
  // Newest first: the last undone entry is at the top, the oldest done entry at the bottom.
  const rows = entries.map((label, index) => ({ label, index })).reverse();

  return (
    <section className="side-panel" aria-labelledby="history-heading">
      <h2 id="history-heading">History</h2>
      {history.dropped && (
        <p className="panel-note" role="status">
          Older entries were dropped: the last {HISTORY_LIMIT} are kept.
        </p>
      )}
      {rows.length === 0 ? (
        <p className="panel-note">No edits yet.</p>
      ) : (
        <ol className="history-list" aria-label="Edits, newest first">
          {rows.map(({ label, index }) => {
            const undone = index > current;
            return (
              <li key={index}>
                <button
                  type="button"
                  className={
                    undone ? "history-entry history-undone" : "history-entry"
                  }
                  aria-current={index === current ? "step" : undefined}
                  disabled={disabled}
                  onClick={() => onGo(index)}
                >
                  {historyLabelText(label)}
                  {undone && <span className="history-mark"> (undone)</span>}
                </button>
              </li>
            );
          })}
        </ol>
      )}
    </section>
  );
}
