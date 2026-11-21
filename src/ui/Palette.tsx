import type { EditorStatus, EditorTool } from "../editor/interaction";
import { DRAWER_ID } from "./drawer/PartsDrawer";

export interface PaletteProps {
  readonly tool: EditorTool;
  /** The parts drawer is open. */
  readonly drawerOpen: boolean;
  readonly onToggleDrawer: () => void;
  readonly disabled?: boolean;
  /**
   * Why the DC rail tool is unavailable (for example, an assignment is open); null or absent
   * when it is available. The button stays visible and says why.
   */
  readonly railUnavailable?: string | null;
  /**
   * Why the Note tool is unavailable (an assignment holds no documentation); null or absent
   * when it is available.
   */
  readonly noteUnavailable?: string | null;
  readonly onSelectTool: (tool: EditorTool) => void;
}

const DRAWING_TOOLS: readonly {
  readonly tool: EditorTool;
  readonly label: string;
  readonly title: string;
}[] = [
  {
    tool: { kind: "wire" },
    label: "Wire",
    title: "Draw an orthogonal wire",
  },
  {
    tool: { kind: "junction" },
    label: "Junction",
    title: "Connect two wires where they cross",
  },
  {
    tool: { kind: "reference" },
    label: "Ground",
    title: "Place a reference (ground) marker on a pin or wire",
  },
];

/**
 * The toolbar's tools: Select, the button that opens the parts drawer (which lists the catalog
 * parts), the DC rail tool, and the wire, junction, and reference tools.
 */
export function Palette({
  tool,
  drawerOpen,
  onToggleDrawer,
  disabled = false,
  railUnavailable = null,
  noteUnavailable = null,
  onSelectTool,
}: PaletteProps) {
  return (
    <div className="palette" role="toolbar" aria-label="Component palette">
      <button
        type="button"
        className="toolbar-button"
        disabled={disabled}
        aria-pressed={tool.kind === "select"}
        onClick={() => onSelectTool({ kind: "select" })}
      >
        Select
      </button>
      <button
        type="button"
        className="toolbar-button"
        disabled={disabled}
        title="Open the parts drawer (shortcut: /)"
        aria-expanded={drawerOpen}
        aria-controls={DRAWER_ID}
        onClick={onToggleDrawer}
      >
        Parts
      </button>
      <button
        type="button"
        className="toolbar-button"
        disabled={disabled || railUnavailable !== null}
        title={
          railUnavailable ??
          "Draw a vertical named DC rail. A rail is a conductor, not a source: it carries no voltage of its own."
        }
        aria-pressed={tool.kind === "rail"}
        onClick={() => onSelectTool({ kind: "rail" })}
      >
        DC rail
      </button>
      {DRAWING_TOOLS.map((entry) => (
        <button
          key={entry.tool.kind}
          type="button"
          className="toolbar-button"
          disabled={disabled}
          title={entry.title}
          aria-pressed={tool.kind === entry.tool.kind}
          onClick={() => onSelectTool(entry.tool)}
        >
          {entry.label}
        </button>
      ))}
      <button
        type="button"
        className="toolbar-button"
        disabled={disabled || noteUnavailable !== null}
        title={
          noteUnavailable ??
          "Place a text note on the drawing. Double-click it to edit the text."
        }
        aria-pressed={tool.kind === "note"}
        onClick={() => onSelectTool({ kind: "note" })}
      >
        Note
      </button>
    </div>
  );
}

/** What the hint line says for the current tool and state. */
export function toolHint(status: EditorStatus): {
  readonly text: string;
  readonly warning: boolean;
} {
  const { tool } = status;

  switch (tool.kind) {
    case "select":
      return {
        text: "Click an object to select it. Drag to move it. Delete removes it. Space or the middle button pans. While a simulation runs, click a switch lever or hold a push button to operate it.",
        warning: false,
      };
    case "place":
      return {
        text: "Click to place. R rotates, Esc stops placing.",
        warning: false,
      };
    case "wire":
      return {
        text: status.routing
          ? "Click for a corner. Click a pin or wire, or the last point again, to finish. Enter finishes here, Esc cancels, Shift bends vertical-first."
          : "Click to start a wire. Corners are added for you. Esc stops drawing.",
        warning: false,
      };
    case "rail":
      return {
        text: status.railStarted
          ? "Click the other end of the rail at the same x. Esc cancels this rail."
          : "Click to fix one end of a vertical DC rail; the line shown is only a preview. Esc stops drawing. A rail is a conductor only: names such as L1, N or PE carry no meaning.",
        warning: false,
      };
    case "note":
      return {
        text: "Click to place a note, then double-click it or press Enter to edit its text. Esc stops placing notes.",
        warning: false,
      };
    case "junction":
      return status.rejected === "crossing"
        ? {
            text: "A junction needs two wires crossing at a point where nothing connects them yet.",
            warning: true,
          }
        : {
            text: "Click a ring where two wires cross to connect them. Esc stops.",
            warning: false,
          };
    case "reference":
      return status.rejected === "attachment"
        ? {
            text: "A ground marker must sit on a pin or a wire, and only one per point.",
            warning: true,
          }
        : {
            text: "Click a pin or a wire to mark it as the reference. Esc stops.",
            warning: false,
          };
  }
}
