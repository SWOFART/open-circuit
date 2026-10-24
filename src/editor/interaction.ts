import type { GridPoint } from "../generated/GridPoint";
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
}

/** A wheel event in canvas pixels; `deltaMode` follows `WheelEvent`. */
export interface CanvasWheel {
  readonly x: number;
  readonly y: number;
  readonly deltaY: number;
  readonly deltaMode: number;
}

/** What a change affects, so the renderer redraws only what it must. */
export type InteractionChange = "view" | "selection" | "pointer";

type Gesture =
  | { readonly kind: "idle" }
  | {
      readonly kind: "panning";
      readonly pointerId: number;
      readonly last: ScenePoint;
    };

/**
 * Select-mode pointer state for the canvas: view transform, pan and zoom
 * gestures, the snapped pointer position, and the single selection. It knows
 * nothing about PixiJS, the DOM, or React, so it is tested directly.
 *
 * The view fits the circuit until the user pans or zooms; after that, resizes
 * and edits keep the user's view until `fitToCircuit` is called.
 */
export class CanvasInteraction {
  private currentView: ViewTransform = EMPTY_VIEW;
  private viewport: Viewport = { width: 0, height: 0 };
  private autoFit = true;
  private scene: SceneModel | null = null;
  private selected: SelectionTarget | null = null;
  private gesture: Gesture = { kind: "idle" };
  private panModifier = false;
  private pointer: GridPoint | null = null;
  // Last pointer position in canvas pixels, to re-snap after a view refit.
  private pointerScreen: ScenePoint | null = null;

  constructor(
    private readonly onChange: (change: InteractionChange) => void = () => {},
  ) {}

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

  /** Replaces the drawn scene; a selection whose object is gone is cleared. */
  setScene(scene: SceneModel): void {
    const first = this.scene === null;
    this.scene = scene;

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

  /** Returns true when a pan starts, so the caller captures the pointer. */
  pointerDown(pointer: CanvasPointer): boolean {
    this.updatePointer(pointer);
    // A new press from the panning pointer means its release was missed
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

    if (pointer.button === LEFT_BUTTON) {
      const hit = this.scene
        ? hitTest(
            this.scene,
            toGrid(pointer, this.currentView),
            HIT_TOLERANCE_PX / this.currentView.scale,
          )
        : null;
      this.select(hit);
    }

    return false;
  }

  pointerMove(pointer: CanvasPointer): void {
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
  }

  pointerUp(pointer: CanvasPointer): void {
    this.endGesture(pointer.pointerId);
    this.updatePointer(pointer);
  }

  /** The pointer was canceled or lost capture: end its gesture. */
  pointerCancel(pointerId: number): void {
    this.endGesture(pointerId);
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
    if (
      this.gesture.kind === "panning" &&
      this.gesture.pointerId === pointerId
    ) {
      this.gesture = { kind: "idle" };
      this.onChange("pointer");
    }
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
    }
  }
}
