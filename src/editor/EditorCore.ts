// The Tauri CSP forbids `unsafe-eval`, and by default PixiJS builds its shader
// helpers with `new Function`. This module provides eval-free replacements and
// must be imported before the first `Application` is created.
import "pixi.js/unsafe-eval";
import { Application, Container, Graphics, Text } from "pixi.js";
import type { Schematic } from "../generated/Schematic";
import type { StableId } from "../generated/StableId";
import { symbols, type SymbolRegistry } from "../symbols";
import { buildScene, type SceneShape } from "./scene";
import { fitView, toScreen, type ViewTransform } from "./view";

const COLORS = {
  background: "#111827",
  grid: 0x263244,
  wire: 0x60a5fa,
  symbol: 0xe5e7eb,
  reference: 0x94a3b8,
  junction: 0x60a5fa,
  label: "#cbd5e1",
} as const;

const STROKE_WIDTH_PX = 2;
const GRID_MIN_SPACING_PX = 10;
// View used before a schematic is set, so the empty canvas still shows a grid.
const EMPTY_VIEW: ViewTransform = { scale: 24, offsetX: 0, offsetY: 0 };

/**
 * Plain TypeScript/PixiJS editor surface. It must not import React: panels
 * talk to it only through the public methods below.
 */
export class EditorCore {
  private readonly application = new Application();
  private readonly world = new Container();
  private destroyed = false;
  private initialized = false;
  private schematic: Schematic | null = null;
  private labels: ReadonlyMap<StableId, string> | undefined;

  constructor(private readonly registry: SymbolRegistry = symbols) {}

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
    this.application.stage.addChild(this.world);
    this.application.renderer.on("resize", this.handleResize);
    this.render();
  }

  /** Shows `schematic`; `labels` maps component IDs to display names. */
  setSchematic(
    schematic: Schematic,
    labels?: ReadonlyMap<StableId, string>,
  ): void {
    this.schematic = schematic;
    this.labels = labels;
    this.render();
  }

  destroy(): void {
    this.destroyed = true;

    if (this.initialized) {
      this.application.renderer.off("resize", this.handleResize);
      this.clearWorld();
      this.application.destroy({ removeView: true }, { children: true });
      this.initialized = false;
    }
  }

  private readonly handleResize = (): void => {
    this.render();
  };

  private clearWorld(): void {
    for (const child of this.world.removeChildren()) {
      child.destroy();
    }
  }

  private render(): void {
    if (!this.initialized || this.destroyed) {
      return;
    }

    this.clearWorld();

    const { width, height } = this.application.screen;
    const scene = this.schematic
      ? buildScene(this.schematic, this.registry, { labels: this.labels })
      : null;
    const view = scene ? fitView(scene.bounds, { width, height }) : EMPTY_VIEW;

    this.world.addChild(this.drawGrid(width, height, view));

    if (scene) {
      for (const shape of scene.shapes) {
        this.world.addChild(this.drawShape(shape, view));
      }
    }
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

  private drawShape(shape: SceneShape, view: ViewTransform): Graphics | Text {
    if (shape.kind === "label") {
      const position = toScreen(shape.position, view);
      const label = new Text({
        text: shape.text,
        style: {
          fill: COLORS.label,
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
          color: COLORS[shape.role],
          width: STROKE_WIDTH_PX,
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
      graphics.fill(COLORS.junction);
    } else {
      graphics.stroke({ color: COLORS.symbol, width: STROKE_WIDTH_PX });
    }

    return graphics;
  }
}
