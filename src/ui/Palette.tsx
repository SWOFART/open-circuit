import type { ComponentCatalog } from "../generated/ComponentCatalog";
import type { EditorTool } from "../editor/interaction";
import { message } from "./messages";

export interface PaletteProps {
  /** The Rust catalog; null until it has loaded. */
  readonly catalog: ComponentCatalog | null;
  readonly tool: EditorTool;
  readonly onSelectTool: (tool: EditorTool) => void;
}

/** Select mode plus one placement tool per component type in the catalog. */
export function Palette({ catalog, tool, onSelectTool }: PaletteProps) {
  return (
    <div className="palette" role="toolbar" aria-label="Component palette">
      <button
        type="button"
        className="toolbar-button"
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
    </div>
  );
}
