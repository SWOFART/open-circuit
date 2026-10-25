// The Tauri CSP forbids `unsafe-eval`, and by default PixiJS builds its shader
// helpers with `new Function`. This module provides eval-free replacements and
// must be imported before the first `Application` is created.
import "pixi.js/unsafe-eval";
import { Application, Container, Graphics, Text } from "pixi.js";
import type { Schematic } from "../generated/Schematic";
import type { StableId } from "../generated/StableId";
import { symbols, type SymbolRegistry } from "../symbols";
import { bindCanvasInput, type CanvasInputBinding } from "./canvasInput";
import {
  commandForIntent,
  dragPreviewSchematic,
  placementPreviewSchematic,
  type EditorCommand,
} from "./commands";
import {
  CanvasInteraction,
  type EditorTool,
  type InteractionChange,
  type InteractionIntent,
} from "./interaction";
import { buildScene, type SceneModel, type SceneShape } from "./scene";
import { toScreen, type ViewTransform } from "./view";

const COLORS = {
  background: "#111827",
  grid: 0x263244,
  wire: 0x60a5fa,
  symbol: 0xe5e7eb,
  reference: 0x94a3b8,
  junction: 0x60a5fa,
  label: "#cbd5e1",
  selected: 0xfbbf24,
  selectedLabel: "#fbbf24",
  cursor: 0x94a3b8,
  preview: 0x93c5fd,
} as const;

const STROKE_WIDTH_PX = 2;
const SELECTED_STROKE_WIDTH_PX = 3;
const GRID_MIN_SPACING_PX = 10;
const CURSOR_ARM_PX = 5;
const PREVIEW_ALPHA = 0.6;

/** How the editor reports document operations and tool changes. */
export interface EditorHost {
  onCommand(command: EditorCommand): void;
  onToolChange?(tool: EditorTool): void;
}

/**
 * Plain TypeScript/PixiJS editor surface. It must not import React: panels
 * talk to it only through the public methods below and the host callbacks.
 */
export class EditorCore {
  private readonly application = new Application();
  private readonly content = new Container();
  private readonly overlay = new Graphics();
  private readonly interaction: CanvasInteraction;
  private destroyed = false;
  private initialized = false;
  private schematic: Schematic | null = null;
  private labels: ReadonlyMap<StableId, string> | undefined;
  private scene: SceneModel | null = null;
  private contentStale = true;
  private overlayStale = true;
  private input: CanvasInputBinding | null = null;

  constructor(
    private readonly host: EditorHost,
    private readonly registry: SymbolRegistry = symbols,
  ) {
    this.interaction = new CanvasInteraction(
      this.handleInteractionChange,
      this.handleIntent,
    );
  }

  async mount(container: HTMLElement): Promise<void> {
    try {
      await this.application.init({
        antialias: true,
        autoDensity: true,
        background: COLORS.background,
        resolution: window.devicePixelRatio || 1,
        resizeTo: container,
      });
    } catch (error) {
      if (!this.destroyed) {
        throw error;
      }
      return;
    }

    if (this.destroyed) {
      this.application.destroy({ removeView: true });
      return;
    }

    this.initialized = true;
    container.appendChild(this.application.canvas);
    this.application.stage.addChild(this.content, this.overlay);
    this.application.renderer.on("resize", this.handleResize);
    this.application.ticker.add(this.flush);
    this.input = bindCanvasInput(
      this.application.canvas,
      this.interaction,
      window,
    );
    this.interaction.setViewport(this.application.screen);
    this.flush();
  }

  /** Shows `schematic`; `labels` maps component IDs to display names. */
  setSchematic(
    schematic: Schematic,
    labels?: ReadonlyMap<StableId, string>,
  ): void {
    this.schematic = schematic;
    this.labels = labels;
    this.scene = buildScene(schematic, this.registry, { labels });
    this.interaction.setScene(this.scene);
    this.contentStale = true;
  }

  /** Selects the Select tool, or the placement tool for a component type. */
  setTool(tool: EditorTool): void {
    this.interaction.setTool(tool);
  }

  /** Frames the whole circuit, as on startup. */
  fitToCircuit(): void {
    this.interaction.fitToCircuit();
  }

  destroy(): void {
    this.destroyed = true;
    this.input?.dispose();
    this.input = null;

    if (this.initialized) {
      this.application.ticker.remove(this.flush);
      this.application.renderer.off("resize", this.handleResize);
      this.clearContent();
      this.application.destroy({ removeView: true }, { children: true });
      this.initialized = false;
    }
  }

  private readonly handleInteractionChange = (
    change: InteractionChange,
  ): void => {
    if (change === "tool") {
      this.input?.refreshCursor();
      this.host.onToolChange?.(this.interaction.tool);
      this.overlayStale = true;
      return;
    }
    if (change !== "pointer") {
      this.contentStale = true;
    }
    if (change !== "selection") {
      this.overlayStale = true;
    }
  };

  private readonly handleIntent = (intent: InteractionIntent): void => {
    if (!this.schematic) {
      return;
    }

    const command = commandForIntent(intent, this.schematic, this.registry);
    if (command) {
      this.host.onCommand(command);
    }
  };

  private readonly handleResize = (): void => {
    this.interaction.setViewport(this.application.screen);
    this.contentStale = true;
    this.overlayStale = true;
  };

  /** Redraws whatever changed; runs once per frame before Pixi renders. */
  private readonly flush = (): void => {
    if (!this.initialized || this.destroyed) {
      return;
    }

    if (this.contentStale) {
      this.contentStale = false;
      this.drawContent();
    }

    if (this.overlayStale) {
      this.overlayStale = false;
      this.drawOverlay();
    }
  };

  private clearContent(): void {
    for (const child of this.content.removeChildren()) {
      child.destroy();
    }
  }

  private drawContent(): void {
    this.clearContent();

    const { width, height } = this.application.screen;
    const view = this.interaction.view;
    const selection = this.interaction.selection;
    const drag = this.interaction.dragPreview;
    const scene =
      drag && this.schematic
        ? buildScene(
            dragPreviewSchematic(this.schematic, drag, this.registry),
            this.registry,
            { labels: this.labels },
          )
        : this.scene;

    this.content.addChild(this.drawGrid(width, height, view));

    if (scene) {
      for (const shape of scene.shapes) {
        const selected =
          selection !== null &&
          shape.owner.kind === selection.kind &&
          shape.owner.id === selection.id;
        this.content.addChild(this.drawShape(shape, view, selected));
      }
    }
  }

  /**
   * Marks the snapped grid point under the pointer and, with a placement tool,
   * draws the component that a click would place there.
   */
  private drawOverlay(): void {
    this.overlay.clear();

    const point = this.interaction.pointerGridPoint;
    if (point === null || this.interaction.isPanning) {
      return;
    }

    const view = this.interaction.view;
    const tool = this.interaction.tool;
    if (tool.kind === "place") {
      const preview = buildScene(
        placementPreviewSchematic(tool.typeId, point, tool.rotationDeg),
        this.registry,
      );
      for (const shape of preview.shapes) {
        this.tracePreviewShape(shape, view);
      }
    }

    const { x, y } = toScreen(point, view);
    this.overlay
      .moveTo(x - CURSOR_ARM_PX, y)
      .lineTo(x + CURSOR_ARM_PX, y)
      .moveTo(x, y - CURSOR_ARM_PX)
      .lineTo(x, y + CURSOR_ARM_PX)
      .stroke({ color: COLORS.cursor, width: 1 });
  }

  private tracePreviewShape(shape: SceneShape, view: ViewTransform): void {
    if (shape.kind === "polyline") {
      const [first, ...rest] = shape.points.map((p) => toScreen(p, view));
      if (!first) {
        return;
      }
      this.overlay.moveTo(first.x, first.y);
      for (const point of rest) {
        this.overlay.lineTo(point.x, point.y);
      }
    } else if (shape.kind === "circle") {
      const center = toScreen(shape.center, view);
      this.overlay.circle(center.x, center.y, shape.radius * view.scale);
    } else {
      return;
    }

    this.overlay.stroke({
      color: COLORS.preview,
      alpha: PREVIEW_ALPHA,
      width: STROKE_WIDTH_PX,
      join: "round",
      cap: "round",
    });
  }

  private drawGrid(
    width: number,
    height: number,
    view: ViewTransform,
  ): Graphics {
    const grid = new Graphics();

    if (view.scale >= GRID_MIN_SPACING_PX) {
      const firstX = Math.ceil(-view.offsetX / view.scale);
      const lastX = Math.floor((width - view.offsetX) / view.scale);
      const firstY = Math.ceil(-view.offsetY / view.scale);
      const lastY = Math.floor((height - view.offsetY) / view.scale);

      for (let gx = firstX; gx <= lastX; gx += 1) {
        const x = gx * view.scale + view.offsetX;
        grid.moveTo(x, 0).lineTo(x, height);
      }

      for (let gy = firstY; gy <= lastY; gy += 1) {
        const y = gy * view.scale + view.offsetY;
        grid.moveTo(0, y).lineTo(width, y);
      }

      grid.stroke({ color: COLORS.grid, width: 1, alpha: 0.65 });
    }

    return grid;
  }

  private drawShape(
    shape: SceneShape,
    view: ViewTransform,
    selected: boolean,
  ): Graphics | Text {
    const strokeWidth = selected ? SELECTED_STROKE_WIDTH_PX : STROKE_WIDTH_PX;

    if (shape.kind === "label") {
      const position = toScreen(shape.position, view);
      const label = new Text({
        text: shape.text,
        style: {
          fill: selected ? COLORS.selectedLabel : COLORS.label,
          fontFamily: "Inter, ui-sans-serif, system-ui, sans-serif",
          fontSize: 13,
        },
      });
      label.anchor.set(0.5);
      label.position.set(position.x, position.y);
      return label;
    }

    const graphics = new Graphics();

    if (shape.kind === "polyline") {
      const [first, ...rest] = shape.points.map((point) =>
        toScreen(point, view),
      );

      if (first) {
        graphics.moveTo(first.x, first.y);
        for (const point of rest) {
          graphics.lineTo(point.x, point.y);
        }
        graphics.stroke({
          color: selected ? COLORS.selected : COLORS[shape.role],
          width: strokeWidth,
          join: "round",
          cap: "round",
        });
      }

      return graphics;
    }

    const center = toScreen(shape.center, view);
    const radius = shape.radius * view.scale;
    graphics.circle(center.x, center.y, radius);

    if (shape.role === "junction") {
      graphics.fill(selected ? COLORS.selected : COLORS.junction);
    } else {
      graphics.stroke({
        color: selected ? COLORS.selected : COLORS.symbol,
        width: strokeWidth,
      });
    }

    return graphics;
  }
}
