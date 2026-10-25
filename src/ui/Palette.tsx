import type { ComponentCatalog } from "../generated/ComponentCatalog";
import type { EditorStatus, EditorTool } from "../editor/interaction";
import { message } from "./messages";

export interface PaletteProps {
  /** The Rust catalog; null until it has loaded. */
  readonly catalog: ComponentCatalog | null;
  readonly tool: EditorTool;
  readonly disabled?: boolean;
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
 * Select mode, one placement tool per component type in the catalog, and the
 * wire, junction, and reference tools.
 */
export function Palette({
  catalog,
  tool,
  disabled = false,
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
      {catalog?.componentTypes.map((entry) => (
        <button
          key={entry.typeId}
          type="button"
          className="toolbar-button"
          disabled={disabled}
          aria-pressed={tool.kind === "place" && tool.typeId === entry.typeId}
          onClick={() =>
            onSelectTool({
              kind: "place",
              typeId: entry.typeId,
              rotationDeg: 0,
            })
          }
        >
          {message(entry.displayNameKey)}
        </button>
      ))}
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
        text: "Click an object to select it. Drag to move it. Delete removes it. Space or the middle button pans.",
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
