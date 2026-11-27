import type { RecoveryStartup } from "../store/recoveryFlow";

type Pending = Exclude<RecoveryStartup, { kind: "none" }>;

export interface RecoveryDialogProps {
  readonly recovery: Pending;
  readonly disabled: boolean;
  /** Why the last attempt failed, to show in the dialog; null when it did not. */
  readonly error: string | null;
  readonly onRestore: () => void;
  readonly onDiscard: () => void;
}

/** A moment as the person who wrote the file reads it: the date and the time, short. */
export function formatRecoveryTime(
  milliseconds: number,
  locale?: string,
  timeZone?: string,
): string {
  return new Date(milliseconds).toLocaleString(locale, {
    dateStyle: "medium",
    timeStyle: "short",
    ...(timeZone ? { timeZone } : {}),
  });
}

/**
 * The offer at start (stage 16 contract, section 9): when the recovery file was written, which
 * project it came from, and the choices Restore and Discard. Escape is not a choice and the
 * dialog waits. A file that cannot be read is offered for deletion with the reason.
 */
export function RecoveryDialog({
  recovery,
  disabled,
  error,
  onRestore,
  onDiscard,
}: RecoveryDialogProps) {
  return (
    <div className="project-dialog-backdrop">
      <section
        className="project-dialog"
        role="alertdialog"
        aria-modal="true"
        aria-labelledby="recovery-dialog-title"
        aria-describedby="recovery-dialog-description"
      >
        {recovery.kind === "offer" ? (
          <>
            <h2 id="recovery-dialog-title">Recover unsaved work?</h2>
            <p id="recovery-dialog-description">
              The program closed before your changes were saved. A copy was
              written on {formatRecoveryTime(recovery.writtenAtMs)}, from{" "}
              {recovery.sourcePath
                ? `the project ${recovery.sourcePath}`
                : "a document that was never saved"}
              . Restoring opens it as a new, unsaved document; your project file
              is not touched.
            </p>
          </>
        ) : (
          <>
            <h2 id="recovery-dialog-title">A recovery file cannot be used</h2>
            <p id="recovery-dialog-description">
              A recovery file was found but cannot be restored:{" "}
              {recovery.reason} Delete it to start clean.
            </p>
          </>
        )}
        {error && (
          <p className="panel-error" role="alert">
            {error}
          </p>
        )}
        <div className="project-dialog-actions">
          {recovery.kind === "offer" && (
            <button
              type="button"
              className="toolbar-button"
              disabled={disabled}
              autoFocus
              onClick={onRestore}
            >
              Restore
            </button>
          )}
          <button
            type="button"
            className="toolbar-button"
            disabled={disabled}
            autoFocus={recovery.kind !== "offer"}
            onClick={onDiscard}
          >
            {recovery.kind === "offer" ? "Discard" : "Delete it"}
          </button>
        </div>
      </section>
    </div>
  );
}
