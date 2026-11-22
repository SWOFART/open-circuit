import { isTauri } from "@tauri-apps/api/core";
import { useEffect, useState } from "react";
import { sheetBounds } from "../editor/sheetRules";
import { bodyBounds } from "../editor/transform";
import type { Diagnostic } from "../generated/Diagnostic";
import type { Schematic } from "../generated/Schematic";
import type { SheetSpec } from "../generated/SheetSpec";
import type { StableId } from "../generated/StableId";
import type { SchematicDocumentStore } from "../store/document";
import { loadSheetDiagnostics } from "../store/engineAdapter";
import { symbols } from "../symbols";
import { describeDiagnostic, message } from "./messages";

interface Props {
  schematic: Schematic;
  store: SchematicDocumentStore;
  selected: SheetSpec | undefined;
  disabled: boolean;
  onSelect: (sheet: SheetSpec, fit?: boolean) => void;
}

export function SheetPanel({
  schematic,
  store,
  selected,
  disabled,
  onSelect,
}: Props) {
  const sheets = schematic.documentation.sheets;
  const index = sheets.findIndex(({ id }) => selected?.id === id);
  const [warningResult, setWarningResult] = useState<{
    schematic: Schematic;
    warnings: Diagnostic[];
  }>();
  const [error, setError] = useState<string | null>(null);
  const [adding, setAdding] = useState(false);
  useEffect(() => {
    if (!isTauri() || sheets.length === 0) return;
    let current = true;
    void loadSheetDiagnostics(
      schematic,
      schematic.components.map((component) => ({
        componentId: component.id,
        ...bodyBounds(component, symbols),
      })),
    ).then(
      (warnings) => {
        if (current) setWarningResult({ schematic, warnings });
      },
      (failure: unknown) => {
        if (current) setError(String(failure));
      },
    );
    return () => {
      current = false;
    };
  }, [schematic, sheets]);

  function select(id: StableId) {
    const sheet = store
      .getState()
      .schematic.documentation.sheets.find((item) => item.id === id);
    if (sheet) onSelect(sheet);
  }

  function reorder(sheet: SheetSpec, offset: -1 | 1) {
    onSelect(sheet, false);
    store.reorderSheet(sheet.id, offset);
  }

  const draft = adding
    ? {
        id: "",
        name: `Sheet ${sheets.length + 1}`,
        size: "a4" as const,
        origin: {
          x: sheets.length
            ? Math.max(...sheets.map((sheet) => sheetBounds(sheet).maxX))
            : 0,
          y: sheets[0]?.origin.y ?? 0,
        },
      }
    : selected;

  return (
    <fieldset disabled={disabled}>
      <legend>{message("drawing.sheets")}</legend>
      {sheets.length > 0 && (
        <label>
          {message("drawing.activeSheet")}
          <select
            value={selected?.id ?? ""}
            onChange={(event) => {
              setAdding(false);
              select(event.currentTarget.value);
            }}
          >
            {sheets.map((sheet, i) => (
              <option key={sheet.id} value={sheet.id}>
                {i + 1}. {sheet.name}
              </option>
            ))}
          </select>
        </label>
      )}
      {selected && (
        <>
          <button type="button" onClick={() => onSelect(selected)}>
            {message("drawing.fitSheet")}
          </button>
          <button
            type="button"
            disabled={index <= 0}
            onClick={() => reorder(selected, -1)}
          >
            {message("drawing.moveSheetEarlier")}
          </button>
          <button
            type="button"
            disabled={index >= sheets.length - 1}
            onClick={() => reorder(selected, 1)}
          >
            {message("drawing.moveSheetLater")}
          </button>
          <button type="button" onClick={() => store.removeSheet(selected.id)}>
            {message("drawing.removeSheet")}
          </button>
        </>
      )}
      <button
        type="button"
        disabled={adding || sheets.length >= 32}
        onClick={() => setAdding(true)}
      >
        {message("drawing.addSheet")}
      </button>
      {draft && (
        <form
          key={JSON.stringify(draft)}
          onSubmit={(event) => {
            event.preventDefault();
            const data = new FormData(event.currentTarget);
            try {
              const id = store.saveSheet(
                {
                  name: String(data.get("name")),
                  size: data.get("size") === "a3" ? "a3" : "a4",
                  origin: {
                    x: Number(data.get("x")),
                    y: Number(data.get("y")),
                  },
                },
                adding ? undefined : draft.id,
              );
              setError(null);
              setAdding(false);
              select(id);
            } catch (failure) {
              setError(String(failure));
            }
          }}
        >
          <label>
            {message("drawing.sheetName")}
            <input name="name" required defaultValue={draft.name} />
          </label>
          <label>
            {message("drawing.pageSize")}
            <select name="size" defaultValue={draft.size}>
              <option value="a4">A4</option>
              <option value="a3">A3</option>
            </select>
          </label>
          <label>
            {message("drawing.sheetX")}
            <input
              name="x"
              type="number"
              step="1"
              required
              defaultValue={draft.origin.x}
            />
          </label>
          <label>
            {message("drawing.sheetY")}
            <input
              name="y"
              type="number"
              step="1"
              required
              defaultValue={draft.origin.y}
            />
          </label>
          <button type="submit">{message("drawing.applySheet")}</button>
          {adding && (
            <button type="button" onClick={() => setAdding(false)}>
              {message("drawing.cancelSheet")}
            </button>
          )}
        </form>
      )}
      {error && <p role="alert">{error}</p>}
      {warningResult?.schematic === schematic &&
        warningResult.warnings.length > 0 && (
          <ul>
            {warningResult.warnings.map((warning, i) => (
              <li key={i}>{describeDiagnostic(warning)}</li>
            ))}
          </ul>
        )}
    </fieldset>
  );
}
