import type { UnsavedChoice } from "../store/projectWorkflow";

interface UnsavedChangesDialogProps {
  readonly closing: boolean;
  readonly disabled: boolean;
  readonly onChoice: (choice: UnsavedChoice) => void;
}

export function UnsavedChangesDialog({
  closing,
  disabled,
  onChoice,
}: UnsavedChangesDialogProps) {
  return (
    <div className="project-dialog-backdrop">
      <section
        className="project-dialog"
        role="alertdialog"
        aria-modal="true"
        aria-labelledby="unsaved-dialog-title"
        aria-describedby="unsaved-dialog-description"
      >
        <h2 id="unsaved-dialog-title">Unsaved changes</h2>
        <p id="unsaved-dialog-description">
          {closing
            ? "Save your changes before closing the window?"
            : "Save your changes before continuing?"}
        </p>
        <div className="project-dialog-actions">
          <button
            type="button"
            className="toolbar-button"
            disabled={disabled}
            onClick={() => onChoice("save")}
          >
            Save
          </button>
          <button
            type="button"
            className="toolbar-button"
            disabled={disabled}
            onClick={() => onChoice("discard")}
          >
            Discard
          </button>
          <button
            type="button"
            className="toolbar-button"
            disabled={disabled}
            onClick={() => onChoice("cancel")}
          >
            Cancel
          </button>
        </div>
      </section>
    </div>
  );
}
