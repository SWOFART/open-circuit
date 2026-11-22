import { isTauri } from "@tauri-apps/api/core";
import { useEffect, useMemo, useState } from "react";
import { analyzeGeometry } from "../editor/connectivity";
import { drawingPng, drawingSvg } from "../editor/drawingExport";
import {
  drawingPage,
  sheetDrawingPage,
  type DrawingPage,
  type PageMark,
} from "../editor/drawingPage";
import { buildScene } from "../editor/scene";
import type { BillOfMaterials } from "../generated/BillOfMaterials";
import type { MaterialParameter } from "../generated/MaterialParameter";
import type { Schematic } from "../generated/Schematic";
import type { SheetSize } from "../generated/SheetSize";
import type { SheetSpec } from "../generated/SheetSpec";
import type { StableId } from "../generated/StableId";
import type { TitleBlock } from "../generated/TitleBlock";
import type { SchematicDocumentStore } from "../store/document";
import { loadBillOfMaterials } from "../store/engineAdapter";
import { ComponentLabels } from "../store/labels";
import { saveDrawing, saveSheetDrawings } from "../store/projectFiles";
import { symbols } from "../symbols";
import { message } from "./messages";
import { unitSymbols } from "./units";
import { SheetPanel } from "./SheetPanel";

function buildDrawings(
  schematic: Schematic,
  size: "auto" | SheetSize,
): DrawingPage[] {
  const scene = buildScene(schematic, symbols, {
    labels: new ComponentLabels().labelsFor(schematic),
    geometry: analyzeGeometry(schematic, symbols),
  });
  const sheets = schematic.documentation.sheets;
  if (sheets.length)
    return sheets.map((sheet, index) =>
      sheetDrawingPage(scene, schematic, sheet, index),
    );
  return [
    drawingPage(
      scene,
      schematic.documentation.titleBlock,
      size === "auto" ? undefined : size,
    ),
  ];
}

const TITLE_FIELDS = {
  title: "drawing.title",
  drawingNumber: "drawing.number",
  revision: "drawing.revision",
  author: "drawing.author",
  company: "drawing.company",
  date: "drawing.date",
} as const satisfies Record<keyof TitleBlock, string>;

function parameterText(parameter: MaterialParameter): string {
  const value =
    parameter.value === null
      ? message("drawing.unset")
      : `${parameter.value} ${unitSymbols[parameter.unit]}`.trim();
  return `${parameter.id}: ${value}`;
}

function materialsPages(materials: BillOfMaterials): DrawingPage[] {
  const lines = [message("drawing.materials")];
  for (const row of materials.components) {
    lines.push(
      `${row.position}. ${row.quantity} × ${message(row.displayNameKey)}`,
    );
    if (row.designators.length > 0) {
      const names = row.designators.join(", ");
      for (let start = 0; start < names.length; start += 100)
        lines.push(names.slice(start, start + 100));
    }
    lines.push(...row.parameters.map(parameterText));
  }
  if (materials.terminalStrips.length > 0)
    lines.push(message("drawing.terminalStrips"));
  for (const row of materials.terminalStrips) {
    lines.push(`${row.strip} (${row.quantity})`);
    const labels = row.labels.join(", ");
    for (let start = 0; start < labels.length; start += 100)
      lines.push(labels.slice(start, start + 100));
  }
  const pages: DrawingPage[] = [];
  for (let start = 0; start < lines.length; start += 36) {
    const marks: PageMark[] = lines
      .slice(start, start + 36)
      .map((text, index) => ({
        kind: "text",
        text,
        position: { x: 148.5, y: 17 + index * 5 },
        size: 3,
      }));
    pages.push({ size: "a4", marks });
  }
  return pages;
}

interface Props {
  readonly schematic: Schematic;
  readonly store: SchematicDocumentStore;
  readonly disabled: boolean;
  readonly onFitSheet: (sheet: SheetSpec) => void;
}

export function DrawingDocumentationPanel({
  schematic,
  store,
  disabled,
  onFitSheet,
}: Props) {
  const [open, setOpen] = useState(false);
  const [size, setSize] = useState<"auto" | SheetSize>("auto");
  const [pending, setPending] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [exportFolder, setExportFolder] = useState<string | null>(null);
  const [selectedSheet, setSelectedSheet] = useState<StableId | null>(null);
  const sheets = schematic.documentation.sheets;
  const active =
    sheets.find((sheet) => sheet.id === selectedSheet) ?? sheets[0];
  const activeIndex = active ? sheets.indexOf(active) : 0;
  const [materials, setMaterials] = useState<{
    schematic: Schematic;
    data: BillOfMaterials;
  } | null>(null);
  const desktop = isTauri();
  const title = schematic.documentation.titleBlock;
  const preview = useMemo<{ svg: string } | { error: string } | null>(() => {
    if (!open) return null;
    try {
      const page = buildDrawings(schematic, size)[activeIndex];
      return { svg: drawingSvg(page) };
    } catch (failure) {
      return { error: String(failure) };
    }
  }, [activeIndex, open, schematic, size]);
  useEffect(() => {
    if (!open || !desktop) return;
    let current = true;
    void loadBillOfMaterials(schematic).then(
      (data) => {
        if (current) setMaterials({ schematic, data });
      },
      (failure: unknown) => {
        if (current) setError(String(failure));
      },
    );
    return () => {
      current = false;
    };
  }, [desktop, open, schematic]);
  const list = materials?.schematic === schematic ? materials.data : null;

  function updateTitle(field: keyof TitleBlock, value: string): boolean {
    if (disabled) return false;
    try {
      store.setTitleBlockField(field, value);
      setError(null);
      return true;
    } catch (failure) {
      setError(String(failure));
      return false;
    }
  }

  async function exportDrawing(format: "svg" | "png" | "pdf") {
    if (disabled || pending) return;
    setError(null);
    setExportFolder(null);
    setPending(true);
    try {
      // Use the saved drawing only, independent of active simulation and editor history.
      const current = store.getState().schematic;
      const pages = buildDrawings(current, size);
      const savePages = async (
        kind: "svg" | "png",
        data: readonly unknown[],
      ) => {
        if (data.length === 1) await saveDrawing(kind, data[0]);
        else setExportFolder(await saveSheetDrawings(kind, data));
      };
      switch (format) {
        case "svg":
          await savePages(format, pages.map(drawingSvg));
          break;
        case "png": {
          const images: number[][] = [];
          for (const page of pages) {
            const image = await drawingPng(page);
            images.push(Array.from(new Uint8Array(await image.arrayBuffer())));
          }
          await savePages(format, images);
          break;
        }
        case "pdf": {
          const data =
            materials?.schematic === current
              ? materials.data
              : await loadBillOfMaterials(current);
          await saveDrawing(format, [...pages, ...materialsPages(data)]);
          break;
        }
      }
    } catch (failure) {
      setError(String(failure));
    } finally {
      setPending(false);
    }
  }

  return (
    <details
      className="side-panel drawing-documentation"
      onToggle={(event) => setOpen(event.currentTarget.open)}
    >
      <summary>{message("drawing.documentation")}</summary>
      {open && (
        <SheetPanel
          schematic={schematic}
          store={store}
          selected={active}
          disabled={disabled || pending}
          onSelect={(sheet) => {
            setSelectedSheet(sheet.id);
            onFitSheet(sheet);
          }}
        />
      )}
      <fieldset disabled={disabled}>
        <legend>{message("drawing.titleBlock")}</legend>
        {(Object.keys(TITLE_FIELDS) as (keyof TitleBlock)[]).map((field) => (
          <label key={field}>
            {message(TITLE_FIELDS[field])}
            <input
              key={title[field]}
              defaultValue={title[field]}
              onBlur={(event) => {
                if (!updateTitle(field, event.currentTarget.value))
                  event.currentTarget.value = title[field];
              }}
              onKeyDown={(event) => {
                if (event.key === "Enter") event.currentTarget.blur();
              }}
            />
          </label>
        ))}
        <button
          type="button"
          onClick={() => {
            const today = new Date();
            const day = String(today.getDate()).padStart(2, "0");
            const month = String(today.getMonth() + 1).padStart(2, "0");
            updateTitle("date", `${today.getFullYear()}-${month}-${day}`);
          }}
        >
          {message("drawing.useToday")}
        </button>
      </fieldset>
      <fieldset disabled={disabled || !desktop || pending}>
        <legend>{message("drawing.export")}</legend>
        {sheets.length === 0 && (
          <label>
            {message("drawing.pageSize")}
            <select
              value={size}
              onChange={(event) =>
                setSize(event.currentTarget.value as typeof size)
              }
            >
              <option value="auto">{message("drawing.autoSize")}</option>
              <option value="a4">A4</option>
              <option value="a3">A3</option>
            </select>
          </label>
        )}
        {(["svg", "png", "pdf"] as const).map((format) => (
          <button
            key={format}
            type="button"
            disabled={preview === null || "error" in preview}
            onClick={() => void exportDrawing(format)}
          >
            {format.toUpperCase()}
          </button>
        ))}
      </fieldset>
      {exportFolder && (
        <p role="status">
          {message("drawing.exportFolder")} {exportFolder}
        </p>
      )}
      {preview &&
        ("svg" in preview ? (
          <img
            className="drawing-preview"
            alt={message("drawing.preview")}
            src={`data:image/svg+xml,${encodeURIComponent(preview.svg)}`}
          />
        ) : (
          <p role="alert">{preview.error}</p>
        ))}
      <h3>{message("drawing.materials")}</h3>
      {list && (
        <>
          {list.components.length === 0 ? (
            <p>{message("drawing.emptyMaterials")}</p>
          ) : (
            <ol>
              {list.components.map((row) => (
                <li key={row.position}>
                  {row.quantity} × {message(row.displayNameKey)}{" "}
                  {row.designators.join(", ")}
                  <ul>
                    {row.parameters.map((parameter) => (
                      <li key={parameter.id}>{parameterText(parameter)}</li>
                    ))}
                  </ul>
                </li>
              ))}
            </ol>
          )}
          {list.terminalStrips.length > 0 && (
            <>
              <h4>{message("drawing.terminalStrips")}</h4>
              <ul>
                {list.terminalStrips.map((row) => (
                  <li key={row.strip}>
                    {row.strip} ({row.quantity}) {row.labels.join(", ")}
                  </li>
                ))}
              </ul>
            </>
          )}
        </>
      )}
      {error && <p role="alert">{error}</p>}
    </details>
  );
}
