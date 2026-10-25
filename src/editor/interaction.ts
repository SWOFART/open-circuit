import type { ComponentTypeId } from "../generated/ComponentTypeId";
import type { GridPoint } from "../generated/GridPoint";
import type { StableId } from "../generated/StableId";
import { EMPTY_GEOMETRY, type EditorGeometry } from "./connectivity";
import type { Rotation } from "./geometry";
import { hitTest, sceneContains, type SelectionTarget } from "./hitTest";
import type { SceneModel, ScenePoint } from "./scene";
import {
  fitView,
  panBy,
  screenToGridPoint,
  toGrid,
  zoomAt,
  type Viewport,
  type ViewTransform,
} from "./view";
import {
  extendRoute,
  isValidWirePath,
  moveVertex,
  samePath,
} from "./wireRoute";

/** Pick radius around the pointer, in screen pixels at any zoom. */
export const HIT_TOLERANCE_PX = 6;
// View used before a schematic is set, so the empty canvas still shows a grid.
export const EMPTY_VIEW: ViewTransform = { scale: 24, offsetX: 0, offsetY: 0 };
// One 100 px mouse-wheel notch zooms by a factor of about 1.22.
const WHEEL_ZOOM_PER_PIXEL = 0.002;
const WHEEL_LINE_PX = 16;

// `PointerEvent.button` values.
const LEFT_BUTTON = 0;
const MIDDLE_BUTTON = 1;

/** A pointer event in canvas pixels (origin at the canvas's top-left). */
export interface CanvasPointer {
  readonly x: number;
  readonly y: number;
  readonly button: number;
  readonly pointerId: number;
  /** Shift held: routes bend vertical-first. Absent means "unchanged". */
  readonly shiftKey?: boolean;
}

/** A wheel event in canvas pixels; `deltaMode` follows `WheelEvent`. */
export interface CanvasWheel {
  readonly x: number;
  readonly y: number;
  readonly deltaY: number;
  readonly deltaMode: number;
}

/** What a change affects, so the renderer redraws only what it must. */
export type InteractionChange =
  "view" | "selection" | "pointer" | "tool" | "preview";

/**
 * The active canvas tool. Placement and the drawing tools stay active for
 * repeated use until Escape.
 */
export type EditorTool =
  | { readonly kind: "select" }
  | {
      readonly kind: "place";
      readonly typeId: ComponentTypeId;
      readonly rotationDeg: Rotation;
    }
  | { readonly kind: "wire" }
  | { readonly kind: "junction" }
  | { readonly kind: "reference" };

/** Why the last click of a drawing tool did nothing, until the pointer moves. */
export type Rejection = "crossing" | "attachment";

/** What the tool bar and hint line show. */
export interface EditorStatus {
  readonly tool: EditorTool;
  /** A wire route is being drawn. */
  readonly routing: boolean;
  readonly rejected: Rejection | null;
}

/**
 * A completed user action, in grid terms. The editor turns it into a document
 * operation; nothing here changes the document directly.
 */
export type InteractionIntent =
  | {
      readonly kind: "place";
      readonly typeId: ComponentTypeId;
      readonly position: GridPoint;
      readonly rotationDeg: Rotation;
    }
  | {
      readonly kind: "move";
      readonly componentId: StableId;
      readonly delta: GridPoint;
    }
  | { readonly kind: "rotate"; readonly componentId: StableId }
  | { readonly kind: "delete"; readonly target: SelectionTarget }
  | { readonly kind: "undo" }
  | { readonly kind: "redo" }
  | { readonly kind: "addWire"; readonly points: readonly GridPoint[] }
  | {
      readonly kind: "editWire";
      readonly wireId: StableId;
      readonly points: readonly GridPoint[];
    }
  | { readonly kind: "addJunction"; readonly position: GridPoint }
  | {
      readonly kind: "moveJunction";
      readonly id: StableId;
      readonly position: GridPoint;
    }
  | { readonly kind: "addReferenceMarker"; readonly position: GridPoint }
  | {
      readonly kind: "moveReferenceMarker";
      readonly id: StableId;
      readonly position: GridPoint;
    };

/** What an uncommitted drag shows. Nothing is committed until release. */
export type DragPreview =
  | {
      readonly kind: "component";
      readonly componentId: StableId;
      readonly delta: GridPoint;
    }
  | {
      readonly kind: "vertex";
      readonly wireId: StableId;
      readonly index: number;
      readonly to: GridPoint;
    }
  | {
      readonly kind: "marker";
      readonly marker: "junction" | "referenceMarker";
      readonly id: StableId;
      readonly to: GridPoint;
      /** Releasing here would land on a valid target. */
      readonly valid: boolean;
    };

type DragTarget =
  | { readonly kind: "component"; readonly id: StableId }
  | {
      readonly kind: "junction" | "referenceMarker";
      readonly id: StableId;
      readonly origin: GridPoint;
    }
  | {
      readonly kind: "vertex";
      readonly wireId: StableId;
      readonly index: number;
      readonly origin: GridPoint;
    };

type Gesture =
  | { readonly kind: "idle" }
  | {
      readonly kind: "panning";
      readonly pointerId: number;
      readonly last: ScenePoint;
    }
  | {
      // Pressed on a draggable object; it becomes a drag once the snapped
      // point moves.
      readonly kind: "pressing" | "dragging";
      readonly pointerId: number;
      readonly target: DragTarget;
      readonly start: GridPoint;
      readonly delta: GridPoint;
    };

function samePoint(a: GridPoint, b: GridPoint): boolean {
  return a.x === b.x && a.y === b.y;
}

/**
 * Pointer and keyboard state for the canvas: view transform, pan and zoom
 * gestures, the snapped pointer position, the single selection, the active
 * tool, drags, and the wire route being drawn. Completed actions are reported
 * as intents. It knows nothing about PixiJS, the DOM, or React, so it is
 * tested directly.
 *
 * The view fits the circuit until the user pans or zooms; after that, resizes
 * and edits keep the user's view until `fitToCircuit` is called.
 */
export class CanvasInteraction {
  private currentView: ViewTransform = EMPTY_VIEW;
  private viewport: Viewport = { width: 0, height: 0 };
  private autoFit = true;
  private scene: SceneModel | null = null;
  private geometry: EditorGeometry = EMPTY_GEOMETRY;
  private selected: SelectionTarget | null = null;
  private gesture: Gesture = { kind: "idle" };
  private panModifier = false;
  private pointer: GridPoint | null = null;
  // Last pointer position in canvas pixels, to re-snap after a view refit.
  private pointerScreen: ScenePoint | null = null;
  private currentTool: EditorTool = { kind: "select" };
  // The committed part of the wire being drawn; its last point is the last
  // waypoint. Null when no route is in progress.
  private route: GridPoint[] | null = null;
  private verticalFirst = false;
  private rejected: Rejection | null = null;

  constructor(
    private readonly onChange: (change: InteractionChange) => void = () => {},
    private readonly onIntent: (intent: InteractionIntent) => void = () => {},
  ) {}

  get tool(): EditorTool {
    return this.currentTool;
  }

  get status(): EditorStatus {
    return {
      tool: this.currentTool,
      routing: this.route !== null,
      rejected: this.rejected,
    };
  }

  /** The drag in progress, or null. */
  get dragPreview(): DragPreview | null {
    const gesture = this.gesture;
    if (gesture.kind !== "dragging") {
      return null;
    }

    const { target, delta } = gesture;
    if (target.kind === "component") {
      return { kind: "component", componentId: target.id, delta };
    }

    const to = { x: target.origin.x + delta.x, y: target.origin.y + delta.y };
    return target.kind === "vertex"
      ? { kind: "vertex", wireId: target.wireId, index: target.index, to }
      : {
          kind: "marker",
          marker: target.kind,
          id: target.id,
          to,
          valid: this.isMarkerTarget(target.kind, to),
        };
  }

  /** The wire being drawn, with the leg to the pointer; null when idle. */
  get routePreview(): GridPoint[] | null {
    if (!this.route) {
      return null;
    }

    return this.pointer
      ? extendRoute(this.route, this.pointer, this.verticalFirst)
      : [...this.route];
  }

  /** The last waypoint of the wire being drawn. */
  get routeAnchor(): GridPoint | null {
    return this.route?.[this.route.length - 1] ?? null;
  }

  get isDragging(): boolean {
    return this.gesture.kind === "dragging";
  }

  get view(): ViewTransform {
    return this.currentView;
  }

  get selection(): SelectionTarget | null {
    return this.selected;
  }

  /** The snapped grid point under the pointer, or null off the canvas. */
  get pointerGridPoint(): GridPoint | null {
    return this.pointer;
  }

  get isPanning(): boolean {
    return this.gesture.kind === "panning";
  }

  get isPanModifierHeld(): boolean {
    return this.panModifier;
  }

  setViewport(viewport: Viewport): void {
    this.viewport = { width: viewport.width, height: viewport.height };

    if (this.autoFit) {
      this.fit();
    }
  }

  /**
   * Replaces the drawn scene and the geometry derived from the same
   * schematic; a selection whose object is gone is cleared.
   */
  setScene(scene: SceneModel, geometry: EditorGeometry = EMPTY_GEOMETRY): void {
    const first = this.scene === null;
    this.scene = scene;
    this.geometry = geometry;

    if (first && this.autoFit) {
      this.fit();
    }

    if (this.selected && !sceneContains(scene, this.selected)) {
      this.selected = null;
      this.onChange("selection");
    }
  }

  /** Frames all schematic geometry and follows resizes until the next pan/zoom. */
  fitToCircuit(): void {
    this.autoFit = true;
    this.fit();
  }

  /** Selects `target` if it is drawn, or clears the selection with null. */
  select(target: SelectionTarget | null): void {
    const next =
      target && this.scene && sceneContains(this.scene, target)
        ? { kind: target.kind, id: target.id }
        : null;

    if (next?.kind === this.selected?.kind && next?.id === this.selected?.id) {
      return;
    }

    this.selected = next;
    this.onChange("selection");
  }

  /** Switches tools; any uncommitted gesture is canceled first. */
  setTool(tool: EditorTool): void {
    this.cancelGesture();
    this.currentTool = tool;
    this.rejected = null;
    if (tool.kind !== "select") {
      this.select(null);
    }
    this.onChange("tool");
  }

  /** Shift: new bends go vertical-first instead of horizontal-first. */
  setVerticalFirst(held: boolean): void {
    if (held !== this.verticalFirst) {
      this.verticalFirst = held;
      if (this.route) {
        this.onChange("preview");
      }
    }
  }

  /**
   * Returns true when a pan or a press on a draggable object starts, so the
   * caller captures the pointer.
   */
  pointerDown(pointer: CanvasPointer): boolean {
    this.noteShift(pointer);
    this.updatePointer(pointer);
    // A new press from the gesture's own pointer means its release was missed
    // (for example, released off the canvas without capture).
    this.endGesture(pointer.pointerId);

    if (this.gesture.kind !== "idle") {
      return false;
    }

    if (
      pointer.button === MIDDLE_BUTTON ||
      (pointer.button === LEFT_BUTTON && this.panModifier)
    ) {
      this.gesture = {
        kind: "panning",
        pointerId: pointer.pointerId,
        last: { x: pointer.x, y: pointer.y },
      };
      this.onChange("pointer");
      return true;
    }

    if (pointer.button !== LEFT_BUTTON) {
      return false;
    }

    const at = screenToGridPoint(pointer, this.currentView);

    switch (this.currentTool.kind) {
      case "place":
        this.onIntent({
          kind: "place",
          typeId: this.currentTool.typeId,
          position: at,
          rotationDeg: this.currentTool.rotationDeg,
        });
        return false;
      case "wire":
        this.routeClick(at);
        return false;
      case "junction":
        if (this.geometry.isJunctionSpot(at)) {
          this.setRejected(null);
          this.onIntent({ kind: "addJunction", position: at });
        } else {
          this.setRejected("crossing");
        }
        return false;
      case "reference":
        if (
          this.geometry.isAttachment(at) &&
          !this.geometry.hasReferenceMarkerAt(at)
        ) {
          this.setRejected(null);
          this.onIntent({ kind: "addReferenceMarker", position: at });
        } else {
          this.setRejected("attachment");
        }
        return false;
      case "select":
        return this.selectPress(pointer, at);
    }
  }

  /** Select mode: pick an object and arm a drag when it can be dragged. */
  private selectPress(pointer: CanvasPointer, at: GridPoint): boolean {
    const exact = toGrid(pointer, this.currentView);
    const tolerance = HIT_TOLERANCE_PX / this.currentView.scale;

    // The handles of the selected wire sit above everything else.
    const vertex = this.vertexUnder(exact, tolerance);
    if (vertex) {
      return this.armDrag(pointer, at, vertex);
    }

    const hit = this.scene ? hitTest(this.scene, exact, tolerance) : null;
    this.select(hit);

    if (hit?.kind === "component") {
      return this.armDrag(pointer, at, { kind: "component", id: hit.id });
    }

    if (hit?.kind === "junction" || hit?.kind === "referenceMarker") {
      const origin = (
        hit.kind === "junction"
          ? this.geometry.junctions
          : this.geometry.referenceMarkers
      ).get(hit.id);

      return origin
        ? this.armDrag(pointer, at, { kind: hit.kind, id: hit.id, origin })
        : false;
    }

    return false;
  }

  private vertexUnder(point: ScenePoint, tolerance: number): DragTarget | null {
    if (this.selected?.kind !== "wire") {
      return null;
    }

    const points = this.geometry.wires.get(this.selected.id);
    const index =
      points?.findIndex(
        (vertex) =>
          Math.hypot(vertex.x - point.x, vertex.y - point.y) <= tolerance,
      ) ?? -1;
    const origin = points?.[index];

    return origin
      ? { kind: "vertex", wireId: this.selected.id, index, origin }
      : null;
  }

  private armDrag(
    pointer: CanvasPointer,
    at: GridPoint,
    target: DragTarget,
  ): boolean {
    this.gesture = {
      kind: "pressing",
      pointerId: pointer.pointerId,
      target,
      start: at,
      delta: { x: 0, y: 0 },
    };
    return true;
  }

  /**
   * A click with the Wire tool: start a route, add a waypoint, or finish. A
   * pin or an existing conductor ends the route at that exact point, and so
   * does clicking the last waypoint again (a double-click).
   */
  private routeClick(at: GridPoint): void {
    if (!this.route) {
      this.route = [at];
      this.onChange("preview");
      this.onChange("tool");
      return;
    }

    const last = this.route[this.route.length - 1];
    if (!last || samePoint(last, at)) {
      this.commitRoute(this.route);
      return;
    }

    const path = extendRoute(this.route, at, this.verticalFirst);
    if (this.geometry.isAttachment(at)) {
      this.commitRoute(path);
      return;
    }

    this.route = path;
    this.onChange("preview");
  }

  /** Enter: finishes the route at the snapped pointer, a free point. */
  finishRoute(): void {
    if (!this.route) {
      return;
    }

    const last = this.route[this.route.length - 1];
    this.commitRoute(
      this.pointer && last && !samePoint(last, this.pointer)
        ? extendRoute(this.route, this.pointer, this.verticalFirst)
        : this.route,
    );
  }

  private commitRoute(path: readonly GridPoint[]): void {
    this.route = null;
    this.onChange("preview");
    this.onChange("tool");

    // A route that never left its start draws nothing.
    if (isValidWirePath(path)) {
      this.onIntent({ kind: "addWire", points: path });
    }
  }

  private isMarkerTarget(
    marker: "junction" | "referenceMarker",
    point: GridPoint,
  ): boolean {
    return marker === "junction"
      ? this.geometry.isJunctionSpot(point)
      : this.geometry.isAttachment(point) &&
          !this.geometry.hasReferenceMarkerAt(point);
  }

  private setRejected(rejected: Rejection | null): void {
    if (rejected !== this.rejected) {
      this.rejected = rejected;
      this.onChange("tool");
    }
  }

  private noteShift(pointer: CanvasPointer): void {
    if (pointer.shiftKey !== undefined) {
      this.setVerticalFirst(pointer.shiftKey);
    }
  }

  pointerMove(pointer: CanvasPointer): void {
    this.noteShift(pointer);

    if (
      this.gesture.kind === "panning" &&
      this.gesture.pointerId === pointer.pointerId
    ) {
      const { last } = this.gesture;
      this.gesture = { ...this.gesture, last: { x: pointer.x, y: pointer.y } };
      this.changeView(
        panBy(this.currentView, pointer.x - last.x, pointer.y - last.y),
      );
    }

    this.updatePointer(pointer);

    if (
      (this.gesture.kind === "pressing" || this.gesture.kind === "dragging") &&
      this.gesture.pointerId === pointer.pointerId &&
      this.pointer
    ) {
      const delta = {
        x: this.pointer.x - this.gesture.start.x,
        y: this.pointer.y - this.gesture.start.y,
      };

      if (
        delta.x !== this.gesture.delta.x ||
        delta.y !== this.gesture.delta.y
      ) {
        this.gesture = { ...this.gesture, kind: "dragging", delta };
        this.onChange("preview");
      }
    }
  }

  /**
   * Ends the gesture. A drag commits one snapped edit if it was released on a
   * valid target; anywhere else it is canceled.
   */
  pointerUp(pointer: CanvasPointer): void {
    this.updatePointer(pointer);

    const gesture = this.gesture;
    const preview = this.dragPreview;
    this.endGesture(pointer.pointerId);

    if (
      gesture.kind === "dragging" &&
      gesture.pointerId === pointer.pointerId &&
      preview &&
      (gesture.delta.x !== 0 || gesture.delta.y !== 0)
    ) {
      this.commitDrag(preview);
    }
  }

  private commitDrag(preview: DragPreview): void {
    switch (preview.kind) {
      case "component":
        this.onIntent({
          kind: "move",
          componentId: preview.componentId,
          delta: preview.delta,
        });
        return;
      case "vertex": {
        const points = this.geometry.wires.get(preview.wireId);
        const edited = points && moveVertex(points, preview.index, preview.to);
        if (points && edited && !samePath(edited, points)) {
          this.onIntent({
            kind: "editWire",
            wireId: preview.wireId,
            points: edited,
          });
        }
        return;
      }
      case "marker":
        if (preview.valid) {
          this.onIntent(
            preview.marker === "junction"
              ? { kind: "moveJunction", id: preview.id, position: preview.to }
              : {
                  kind: "moveReferenceMarker",
                  id: preview.id,
                  position: preview.to,
                },
          );
        }
        return;
    }
  }

  /**
   * The pointer was canceled or lost capture: end its gesture without
   * committing anything.
   */
  pointerCancel(pointerId: number): void {
    this.endGesture(pointerId);
  }

  /**
   * Escape cancels an uncommitted gesture or wire route; otherwise it returns
   * a placement or drawing tool to Select mode.
   */
  escape(): void {
    if (!this.cancelGesture() && this.currentTool.kind !== "select") {
      this.setTool({ kind: "select" });
    }
  }

  /** R: rotates the placement preview or the selected component clockwise. */
  rotate(): void {
    if (this.currentTool.kind === "place") {
      this.currentTool = {
        ...this.currentTool,
        rotationDeg: ((this.currentTool.rotationDeg + 90) % 360) as Rotation,
      };
      this.onChange("tool");
      return;
    }

    if (
      this.gesture.kind === "idle" &&
      !this.route &&
      this.selected?.kind === "component"
    ) {
      this.onIntent({ kind: "rotate", componentId: this.selected.id });
    }
  }

  /** Delete or Backspace: removes the selected object. */
  deleteSelection(): void {
    if (this.gesture.kind === "idle" && !this.route && this.selected) {
      this.onIntent({ kind: "delete", target: this.selected });
    }
  }

  /** Undo during an uncommitted gesture only cancels that gesture. */
  undo(): void {
    if (!this.cancelGesture()) {
      this.onIntent({ kind: "undo" });
    }
  }

  /** Like undo, redo first cancels an uncommitted gesture. */
  redo(): void {
    if (!this.cancelGesture()) {
      this.onIntent({ kind: "redo" });
    }
  }

  pointerLeave(): void {
    this.pointerScreen = null;
    if (this.pointer !== null) {
      this.pointer = null;
      this.onChange("pointer");
    }
  }

  /** Zooms around the pointer. */
  wheel(wheel: CanvasWheel): void {
    const unit =
      wheel.deltaMode === 1
        ? WHEEL_LINE_PX
        : wheel.deltaMode === 2
          ? Math.max(this.viewport.height, WHEEL_LINE_PX)
          : 1;
    const factor = Math.exp(-wheel.deltaY * unit * WHEEL_ZOOM_PER_PIXEL);

    this.changeView(zoomAt(this.currentView, wheel, factor));
  }

  /** Space held: a left-button drag pans instead of selecting. */
  setPanModifier(held: boolean): void {
    this.panModifier = held;
  }

  private endGesture(pointerId: number): void {
    if (this.gesture.kind !== "idle" && this.gesture.pointerId === pointerId) {
      this.cancelGesture();
    }
  }

  /**
   * Drops the pointer gesture, or else the wire route, without committing.
   * Returns true if there was one.
   */
  private cancelGesture(): boolean {
    const previous = this.gesture;
    if (previous.kind === "idle") {
      if (!this.route) {
        return false;
      }

      this.route = null;
      this.onChange("preview");
      this.onChange("tool");
      return true;
    }

    this.gesture = { kind: "idle" };
    if (previous.kind === "dragging") {
      this.onChange("preview");
    }
    this.onChange("pointer");
    return true;
  }

  private fit(): void {
    this.currentView = this.scene
      ? fitView(this.scene.bounds, this.viewport)
      : EMPTY_VIEW;
    this.onChange("view");

    if (this.pointerScreen) {
      this.updatePointer(this.pointerScreen);
    }
  }

  private changeView(view: ViewTransform): void {
    if (
      view.scale === this.currentView.scale &&
      view.offsetX === this.currentView.offsetX &&
      view.offsetY === this.currentView.offsetY
    ) {
      return;
    }

    this.autoFit = false;
    this.currentView = view;
    this.onChange("view");
  }

  private updatePointer(position: ScenePoint): void {
    this.pointerScreen = { x: position.x, y: position.y };
    const next = screenToGridPoint(position, this.currentView);

    if (next.x !== this.pointer?.x || next.y !== this.pointer?.y) {
      this.pointer = next;
      this.onChange("pointer");
      // A refused click stays explained only until the pointer moves on.
      this.setRejected(null);
    }
  }
}
