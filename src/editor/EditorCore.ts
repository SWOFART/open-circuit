// The Tauri CSP forbids `unsafe-eval`, and by default PixiJS builds its shader
// helpers with `new Function`. This module provides eval-free replacements and
// must be imported before the first `Application` is created.
import "pixi.js/unsafe-eval";
import { Application, Container, Graphics, Text } from "pixi.js";
import type { ComponentTypeId } from "../generated/ComponentTypeId";
import type { GridPoint } from "../generated/GridPoint";
import type { DeviceState } from "../generated/DeviceState";
import type { Schematic } from "../generated/Schematic";
import { sheetBounds } from "./sheetRules";
import type { StableId } from "../generated/StableId";
import { symbols, type SymbolRegistry } from "../symbols";
import { bindCanvasInput, type CanvasInputBinding } from "./canvasInput";
import {
  clipboardBounds,
  clipboardDrawing,
  PASTE_OFFSET,
  translateClipboard,
  type ClipboardSchematic,
} from "./clipboard";
import {
  commandForIntent,
  dragPreviewSchematic,
  placementPreviewSchematic,
  type EditorCommand,
} from "./commands";
import {
  analyzeGeometry,
  EMPTY_GEOMETRY,
  type EditorGeometry,
} from "./connectivity";
import type { SelectionTarget } from "./hitTest";
import { operatingTargets } from "./operate";
import {
  CanvasInteraction,
  type ClipboardAction,
  type EditorStatus,
  type EditorTool,
  type InteractionChange,
  type InteractionIntent,
  type OperateAction,
} from "./interaction";
import {
  boundsOf,
  buildScene,
  JUNCTION_RADIUS,
  referenceMarkerShapes,
  type SceneModel,
  type SceneShape,
} from "./scene";
import { screenToGridPoint, toScreen, type ViewTransform } from "./view";

const COLORS = {
  background: "#111827",
  grid: 0x263244,
  wire: 0x60a5fa,
  rail: 0x34d399,
  symbol: 0xe5e7eb,
  reference: 0x94a3b8,
  junction: 0x60a5fa,
  label: "#cbd5e1",
  selected: 0xfbbf24,
  selectedLabel: "#fbbf24",
  detached: 0xf87171,
  cursor: 0x94a3b8,
  preview: 0x93c5fd,
} as const;

const STROKE_WIDTH_PX = 2;
const SELECTED_STROKE_WIDTH_PX = 3;
const GRID_MIN_SPACING_PX = 10;
const CURSOR_ARM_PX = 5;
const PREVIEW_ALPHA = 0.6;
const HANDLE_HALF_PX = 4;
const RING_RADIUS_PX = 7;
// Never stored: the reference preview is drawn, not added to the document.
const PREVIEW_OWNER = {
  kind: "referenceMarker",
  id: "00000000-0000-4000-8000-000000000000",
} as const;

/** How the editor reports document operations and tool changes. */
export interface EditorHost {
  onCommand(command: EditorCommand): void;
  onProbe?(position: GridPoint): void;
  /**
   * Copy, cut, paste or duplicate was asked for, with the selection at that moment. The host
   * owns the system clipboard and answers with `startPaste` or a command.
   */
  onClipboard?(
    action: ClipboardAction,
    targets: readonly SelectionTarget[],
  ): void;
  /** The tool, an unfinished wire route, or a refused click changed. */
  onStatusChange?(status: EditorStatus): void;
  /**
   * The user toggled a switch, or pressed or released a push button. The host sends the
   * position to the Engine: the editor never decides what conducts.
   */
  onOperate?(componentId: StableId, action: OperateAction): void;
  /**
   * A double press or Enter asked for the editor of a component or a rail. `anchor` is the
   * object's position in canvas pixels, to place the editor next to it.
   */
  onOpenEditor?(
    target: SelectionTarget,
    anchor: { readonly x: number; readonly y: number } | null,
  ): void;
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
  /** Rails that diagnostics name; drawn as warnings. */
  private flaggedRails: ReadonlySet<StableId> = new Set();
  // The device states of the run on screen; an empty map draws every part at rest.
  private deviceStates: ReadonlyMap<StableId, DeviceState> = new Map();
  private deviceStatesKey = "";
  private operable = false;
  private probeMarkers: readonly (GridPoint | null)[] = [];
  private scene: SceneModel | null = null;
  private geometry: EditorGeometry = EMPTY_GEOMETRY;
  private contentStale = true;
  private overlayStale = true;
  private input: CanvasInputBinding | null = null;
  // A selection asked for before the scene contained the object; applied by the next scene.
  private pendingSelection: readonly SelectionTarget[] | null = null;

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
    // Focusable so the keyboard can return to the canvas (the drawer's Shift+Enter, Escape).
    this.application.canvas.tabIndex = 0;
    this.application.canvas.setAttribute("aria-label", "Circuit canvas");
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
    this.geometry = analyzeGeometry(schematic, this.registry);
    this.scene = buildScene(schematic, this.registry, {
      labels,
      geometry: this.geometry,
      states: this.deviceStates,
      flaggedRails: this.flaggedRails,
    });
    this.interaction.setScene(this.scene, this.geometry);
    if (this.pendingSelection) {
      const pending = this.pendingSelection;
      this.pendingSelection = null;
      this.interaction.selectMany(pending);
    }
    this.syncOperable();
    this.contentStale = true;
    this.overlayStale = true;
  }

  /** Marks the rails that the Engine's diagnostics name; an empty set clears the marks. */
  setFlaggedRails(ids: ReadonlySet<StableId>): void {
    const same =
      ids.size === this.flaggedRails.size &&
      [...ids].every((id) => this.flaggedRails.has(id));

    if (same) {
      return;
    }

    this.flaggedRails = ids;

    if (this.schematic) {
      this.scene = buildScene(this.schematic, this.registry, {
        labels: this.labels,
        geometry: this.geometry,
        states: this.deviceStates,
        flaggedRails: this.flaggedRails,
      });
      this.interaction.setScene(this.scene, this.geometry);
      this.contentStale = true;
    }
  }

  /**
   * Shows the discrete states the Engine reported for the run on screen (none draws every
   * part at rest), and says whether a press operates a switch or a push button: it does
   * only while a run is on screen. Redraws only when a state changed.
   */
  setRunState(states: readonly DeviceState[] | null, operable: boolean): void {
    const next = new Map<StableId, DeviceState>(
      (states ?? []).map((state) => [state.componentId, state]),
    );
    const key = JSON.stringify([...next.values()]);

    if (key !== this.deviceStatesKey) {
      this.deviceStatesKey = key;
      this.deviceStates = next;

      if (this.schematic) {
        this.scene = buildScene(this.schematic, this.registry, {
          labels: this.labels,
          geometry: this.geometry,
          states: this.deviceStates,
          flaggedRails: this.flaggedRails,
        });
        this.interaction.setScene(this.scene, this.geometry);
        this.contentStale = true;
      }
    }

    if (operable !== this.operable) {
      this.operable = operable;
      this.syncOperable();
    }
  }

  private syncOperable(): void {
    this.interaction.setOperable(
      this.operable && this.schematic
        ? operatingTargets(this.schematic, this.registry)
        : [],
    );
  }

  /** Draws session probe targets without editing the schematic. */
  setProbeMarkers(positions: readonly (GridPoint | null)[]): void {
    this.probeMarkers = positions;
    this.overlayStale = true;
  }

  /** Selects the active canvas tool. */
  setTool(tool: EditorTool): void {
    this.interaction.setTool(tool);
  }

  /**
   * Places one part with the catalog defaults at the grid point under the client position
   * (a pointer drop) or, with no position, at the center of the visible canvas. This is the
   * ordinary placement command; the active tool is not changed. Returns false when nothing
   * was placed (the canvas is not ready, or the position is outside it).
   */
  placeAt(
    typeId: ComponentTypeId,
    client?: { readonly x: number; readonly y: number },
  ): boolean {
    if (!this.initialized || this.destroyed || !this.schematic) {
      return false;
    }

    const { width, height } = this.application.screen;
    let point = { x: width / 2, y: height / 2 };

    if (client) {
      const rect = this.application.canvas.getBoundingClientRect();
      point = { x: client.x - rect.left, y: client.y - rect.top };

      if (point.x < 0 || point.y < 0 || point.x > width || point.y > height) {
        return false;
      }
    }

    this.handleIntent({
      kind: "place",
      typeId,
      position: screenToGridPoint(point, this.interaction.view),
      rotationDeg: 0,
    });
    return true;
  }

  /** Moves the keyboard focus to the canvas. */
  focusCanvas(): void {
    if (this.initialized && !this.destroyed) {
      this.application.canvas.focus({ preventScroll: true });
    }
  }

  /** Frames the whole circuit, as on startup. */
  /** Selects an object of the drawing, for a panel that lists them; null clears. */
  select(target: SelectionTarget | null): void {
    this.interaction.select(target);

    // An object added by the command of this same turn is not in the scene until the next
    // `setSchematic`, which then completes the selection.
    this.pendingSelection =
      target !== null &&
      (this.interaction.selection?.kind !== target.kind ||
        this.interaction.selection?.id !== target.id)
        ? [target]
        : null;
  }

  /** Selects several objects, such as the ones a paste just added; null clears. */
  selectMany(targets: readonly SelectionTarget[]): void {
    this.interaction.selectMany(targets);

    // Objects added by the command of this same turn join the selection with the next scene.
    this.pendingSelection =
      targets.length > this.interaction.selections.length ? targets : null;
  }

  /** Whether the document still holds an object with each of these IDs. */
  private holdsAll(schematic: ClipboardSchematic): boolean {
    const here = this.schematic;
    if (!here) return false;
    const has = (list: readonly { readonly id: StableId }[], id: StableId) =>
      list.some((item) => item.id === id);
    return (
      schematic.components.every(({ id }) => has(here.components, id)) &&
      schematic.wires.every(({ id }) => has(here.wires, id)) &&
      schematic.junctions.every(({ id }) => has(here.junctions, id)) &&
      schematic.referenceMarkers.every(({ id }) =>
        has(here.referenceMarkers, id),
      ) &&
      schematic.rails.every(({ id }) => has(here.rails, id)) &&
      schematic.notes.every(({ id }) => has(here.documentation.notes, id))
    );
  }

  /**
   * Starts placing a paste (contract section 4.4): a translucent preview whose top-left corner
   * follows the pointer; a click or Enter commits it, Escape cancels. It first shows offset
   * (+2, +2) from its source when the source is still in the document, and centered in the view
   * otherwise. Returns false when there is nothing to place.
   */
  startPaste(schematic: ClipboardSchematic): boolean {
    const bounds = clipboardBounds(schematic, this.registry);
    if (!this.initialized || this.destroyed || !bounds) {
      return false;
    }

    let delta: GridPoint = PASTE_OFFSET;
    if (!this.holdsAll(schematic)) {
      const { width, height } = this.application.screen;
      const center = screenToGridPoint(
        { x: width / 2, y: height / 2 },
        this.interaction.view,
      );
      delta = {
        x: center.x - Math.round((bounds.minX + bounds.maxX) / 2),
        y: center.y - Math.round((bounds.minY + bounds.maxY) / 2),
      };
    }

    this.interaction.startPaste({
      schematic,
      anchor: { x: bounds.minX, y: bounds.minY },
      delta,
    });
    this.focusCanvas();
    return true;
  }

  /**
   * Selects one object and centers the view on it, keeping the zoom (canvas search). Returns
   * false when the drawing has no such object.
   */
  reveal(target: SelectionTarget): boolean {
    const shapes = (this.scene?.shapes ?? []).filter(
      (shape) =>
        shape.owner.kind === target.kind && shape.owner.id === target.id,
    );
    if (shapes.length === 0) {
      return false;
    }

    this.select(target);
    this.interaction.centerOn(boundsOf(shapes));
    return true;
  }

  fitToCircuit(): void {
    this.interaction.fitToCircuit();
  }

  fitToBounds(bounds: SceneModel["bounds"]): void {
    this.interaction.fitToBounds(bounds);
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
      this.host.onStatusChange?.(this.interaction.status);
      this.overlayStale = true;
      return;
    }
    if (change !== "pointer") {
      this.contentStale = true;
    }
    if (change === "selection") {
      this.host.onStatusChange?.(this.interaction.status);
    } else {
      this.overlayStale = true;
    }
  };

  private readonly handleIntent = (intent: InteractionIntent): void => {
    if (!this.schematic) {
      return;
    }

    if (intent.kind === "probe") {
      this.host.onProbe?.(intent.position);
      return;
    }

    if (intent.kind === "operate") {
      this.host.onOperate?.(intent.componentId, intent.action);
      return;
    }

    if (intent.kind === "openEditor") {
      const component =
        intent.target.kind === "component"
          ? this.schematic.components.find(({ id }) => id === intent.target.id)
          : undefined;
      this.host.onOpenEditor?.(
        intent.target,
        component ? toScreen(component.position, this.interaction.view) : null,
      );
      return;
    }

    if (intent.kind === "clipboard") {
      this.host.onClipboard?.(intent.action, intent.targets);
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
    const selections = this.interaction.selections;
    const drag = this.interaction.dragPreview;

    // During a drag the drawing shows where the release would leave it.
    let shown = this.schematic;
    let scene = this.scene;
    let geometry = this.geometry;
    if (drag && this.schematic) {
      shown = dragPreviewSchematic(this.schematic, drag, this.registry);
      geometry = analyzeGeometry(shown, this.registry);
      scene = buildScene(shown, this.registry, {
        labels: this.labels,
        geometry,
        states: this.deviceStates,
        flaggedRails: this.flaggedRails,
      });
    }

    this.content.addChild(this.drawGrid(width, height, view));

    if (shown?.documentation.sheets.length) {
      const frames = new Graphics();
      for (const sheet of shown.documentation.sheets) {
        const bounds = sheetBounds(sheet);
        const first = toScreen({ x: bounds.minX, y: bounds.minY }, view);
        const last = toScreen({ x: bounds.maxX, y: bounds.maxY }, view);
        frames
          .rect(first.x, first.y, last.x - first.x, last.y - first.y)
          .stroke({ color: COLORS.preview, width: 1, alpha: 0.65 });
      }
      this.content.addChild(frames);
    }

    if (scene) {
      for (const shape of scene.shapes) {
        // Every member of the selection is highlighted; only the primary one has handles.
        const selected = selections.some(
          (member) =>
            shape.owner.kind === member.kind && shape.owner.id === member.id,
        );
        this.content.addChild(this.drawShape(shape, view, selected));
      }

      for (const dot of scene.connectionDots) {
        this.content.addChild(this.drawDot(dot, view));
      }
    }

    // Drag handles of the selected wire, at the vertices as they are now.
    const handles =
      selection?.kind === "wire" && shown
        ? geometry.wires.get(selection.id)
        : selection?.kind === "rail" && shown
          ? geometry.rails.get(selection.id)
          : null;
    if (handles) {
      this.content.addChild(this.drawHandles(handles, view));
    }
  }

  /**
   * The pointer's snapped grid point, plus what the active tool would do
   * there: the component a click would place, the route being drawn, the
   * junction or reference marker a click would add, or why a drop is refused.
   */
  private drawOverlay(): void {
    this.overlay.clear();

    const view = this.interaction.view;
    const tool = this.interaction.tool;
    const drag = this.interaction.dragPreview;

    this.probeMarkers.forEach((position, index) => {
      if (position)
        this.traceRing(position, view, 0.5, index === 0 ? 0xfb7185 : 0xe2e8f0);
    });

    if (tool.kind === "junction") {
      for (const spot of this.geometry.junctionSpots) {
        this.traceRing(spot, view, 0.5);
      }
    }

    const route = this.interaction.routePreview;
    if (route) {
      this.traceRoute(route, view);
    }

    // The rectangle of a box selection; nothing is selected until it is released.
    const box = this.interaction.boxPreview;
    if (box) {
      const first = toScreen({ x: box.minX, y: box.minY }, view);
      const last = toScreen({ x: box.maxX, y: box.maxY }, view);
      this.overlay
        .rect(first.x, first.y, last.x - first.x, last.y - first.y)
        .fill({ color: COLORS.preview, alpha: 0.12 })
        .stroke({ color: COLORS.preview, width: 1, alpha: 0.8 });
    }

    // The rail the next click would draw; red when a click would not commit it.
    const rail = this.interaction.railPreview;
    if (rail) {
      this.traceRoute(
        [
          { x: rail.x, y: rail.top },
          { x: rail.x, y: rail.bottom },
        ],
        view,
        rail.valid ? COLORS.preview : COLORS.detached,
      );
    }

    // The paste being placed, translucent, where a click would put it.
    const pasting = this.interaction.pastePreview;
    if (pasting) {
      for (const shape of buildScene(
        clipboardDrawing(translateClipboard(pasting.schematic, pasting.delta)),
        this.registry,
      ).shapes) {
        this.tracePreviewShape(shape, view);
      }
    }

    const point = this.interaction.pointerGridPoint;
    if (drag?.kind === "marker" && !drag.valid && point) {
      this.traceRing(point, view, 1, COLORS.detached);
    }

    if (point === null || this.interaction.isPanning) {
      return;
    }

    switch (tool.kind) {
      case "place":
        for (const shape of buildScene(
          placementPreviewSchematic(tool.typeId, point, tool.rotationDeg),
          this.registry,
        ).shapes) {
          this.tracePreviewShape(shape, view);
        }
        break;
      case "junction":
        if (this.geometry.isJunctionSpot(point)) {
          const { x, y } = toScreen(point, view);
          this.overlay
            .circle(x, y, JUNCTION_RADIUS * view.scale * 1.6)
            .fill({ color: COLORS.preview, alpha: PREVIEW_ALPHA });
        }
        break;
      case "reference":
        if (
          this.geometry.isAttachment(point) &&
          !this.geometry.hasReferenceMarkerAt(point)
        ) {
          for (const shape of referenceMarkerShapes(point, PREVIEW_OWNER)) {
            this.tracePreviewShape(shape, view);
          }
        }
        break;
      case "wire":
        // A pin or a wire under the pointer would end the route there.
        if (this.geometry.isAttachment(point)) {
          this.traceRing(point, view, 0.9);
        }
        break;
      case "select":
        break;
    }

    const { x, y } = toScreen(point, view);
    this.overlay
      .moveTo(x - CURSOR_ARM_PX, y)
      .lineTo(x + CURSOR_ARM_PX, y)
      .moveTo(x, y - CURSOR_ARM_PX)
      .lineTo(x, y + CURSOR_ARM_PX)
      .stroke({ color: COLORS.cursor, width: 1 });
  }

  private traceRing(
    point: { readonly x: number; readonly y: number },
    view: ViewTransform,
    alpha: number,
    color: number = COLORS.preview,
  ): void {
    const { x, y } = toScreen(point, view);
    this.overlay
      .circle(x, y, RING_RADIUS_PX)
      .stroke({ color, alpha, width: STROKE_WIDTH_PX });
  }

  private traceRoute(
    route: readonly { readonly x: number; readonly y: number }[],
    view: ViewTransform,
    color: number = COLORS.preview,
  ): void {
    const [first, ...rest] = route.map((point) => toScreen(point, view));
    if (!first) {
      return;
    }

    this.overlay.moveTo(first.x, first.y);
    for (const point of rest) {
      this.overlay.lineTo(point.x, point.y);
    }
    this.overlay.stroke({
      color,
      width: STROKE_WIDTH_PX,
      join: "round",
      cap: "round",
    });

    // The start and the last waypoint are where the route is anchored.
    const anchor = this.interaction.routeAnchor;
    for (const point of [route[0], anchor]) {
      if (point) {
        const { x, y } = toScreen(point, view);
        this.overlay.circle(x, y, 3).fill(COLORS.preview);
      }
    }
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

  /** A derived connection dot, drawn like an explicit junction. */
  private drawDot(
    point: { readonly x: number; readonly y: number },
    view: ViewTransform,
  ): Graphics {
    const { x, y } = toScreen(point, view);

    return new Graphics()
      .circle(x, y, JUNCTION_RADIUS * view.scale)
      .fill(COLORS.junction);
  }

  private drawHandles(
    points: readonly { readonly x: number; readonly y: number }[],
    view: ViewTransform,
  ): Graphics {
    const handles = new Graphics();

    for (const point of points) {
      const { x, y } = toScreen(point, view);
      handles
        .rect(
          x - HANDLE_HALF_PX,
          y - HANDLE_HALF_PX,
          2 * HANDLE_HALF_PX,
          2 * HANDLE_HALF_PX,
        )
        .fill(COLORS.selected)
        .stroke({ color: COLORS.background, width: 1 });
    }

    return handles;
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
    // A marker with nothing under it, or a rail that a diagnostic names, is drawn as a
    // warning, not hidden.
    const color = selected
      ? COLORS.selected
      : shape.detached || shape.flagged
        ? COLORS.detached
        : null;

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
          color: color ?? COLORS[shape.role],
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
      graphics.fill(color ?? COLORS.junction);
    } else if (shape.role === "railEnd") {
      graphics.fill(color ?? COLORS.rail);
    } else {
      graphics.stroke({
        color: color ?? COLORS.symbol,
        width: strokeWidth,
      });
    }

    return graphics;
  }
}
