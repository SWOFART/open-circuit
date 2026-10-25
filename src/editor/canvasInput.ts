import type { CanvasInteraction, CanvasPointer } from "./interaction";

/** The parts of the canvas element the input binding uses. */
export interface CanvasInputSurface extends EventTarget {
  getBoundingClientRect(): { readonly left: number; readonly top: number };
  setPointerCapture(pointerId: number): void;
  releasePointerCapture(pointerId: number): void;
  hasPointerCapture(pointerId: number): boolean;
  readonly style: { cursor: string };
}

const MIDDLE_BUTTON = 1;
const NON_TEXT_INPUT_TYPES = new Set([
  "button",
  "checkbox",
  "color",
  "file",
  "image",
  "radio",
  "range",
  "reset",
  "submit",
]);

/** Shortcuts never act while the user is typing in a text-entry field. */
export function isTextEntryTarget(target: EventTarget | null): boolean {
  if (target === null || !("tagName" in target)) {
    return false;
  }

  const element = target as HTMLElement;
  if (element.isContentEditable) {
    return true;
  }

  switch (String(element.tagName).toUpperCase()) {
    case "TEXTAREA":
    case "SELECT":
      return true;
    case "INPUT":
      return !NON_TEXT_INPUT_TYPES.has(
        String((element as HTMLInputElement).type).toLowerCase(),
      );
    default:
      return false;
  }
}

export interface CanvasInputBinding {
  /** Re-derives the cursor after a change made outside this binding. */
  refreshCursor(): void;
  /** Removes every listener and resets the cursor. */
  dispose(): void;
}

/**
 * Feeds canvas pointer and wheel events, plus the Space pan modifier and the
 * editing shortcuts from `keyboard` (normally `window`), into `interaction`.
 * Only events aimed at the canvas are consumed, so the React panels keep their
 * own input, and no shortcut acts while a text-entry field has focus.
 */
export function bindCanvasInput(
  surface: CanvasInputSurface,
  interaction: CanvasInteraction,
  keyboard: EventTarget,
): CanvasInputBinding {
  const local = (event: PointerEvent): CanvasPointer => {
    const rect = surface.getBoundingClientRect();

    return {
      x: event.clientX - rect.left,
      y: event.clientY - rect.top,
      button: event.button,
      pointerId: event.pointerId,
      shiftKey: event.shiftKey,
    };
  };

  const syncCursor = (): void => {
    surface.style.cursor = interaction.isPanning
      ? "grabbing"
      : interaction.isPanModifierHeld
        ? "grab"
        : interaction.isDragging
          ? "move"
          : interaction.tool.kind !== "select"
            ? "crosshair"
            : "";
  };

  const onPointerDown = (event: PointerEvent): void => {
    if (interaction.pointerDown(local(event))) {
      event.preventDefault();
      try {
        surface.setPointerCapture(event.pointerId);
      } catch {
        // The pointer is no longer active (NotFoundError). The pan still
        // works while the pointer stays over the canvas.
      }
    }
    syncCursor();
  };

  const onPointerMove = (event: PointerEvent): void => {
    interaction.pointerMove(local(event));
    syncCursor();
  };

  const onPointerUp = (event: PointerEvent): void => {
    interaction.pointerUp(local(event));
    if (surface.hasPointerCapture(event.pointerId)) {
      surface.releasePointerCapture(event.pointerId);
    }
    syncCursor();
  };

  const onPointerCancel = (event: PointerEvent): void => {
    interaction.pointerCancel(event.pointerId);
    syncCursor();
  };

  const onPointerLeave = (): void => {
    interaction.pointerLeave();
  };

  // Keeps Windows' middle-button autoscroll from starting over the canvas.
  const onMouseDown = (event: MouseEvent): void => {
    if (event.button === MIDDLE_BUTTON) {
      event.preventDefault();
    }
  };

  const onWheel = (event: WheelEvent): void => {
    event.preventDefault();
    const rect = surface.getBoundingClientRect();
    interaction.wheel({
      x: event.clientX - rect.left,
      y: event.clientY - rect.top,
      deltaY: event.deltaY,
      deltaMode: event.deltaMode,
    });
  };

  /**
   * Editing shortcuts from docs/contracts/stage-2-editor.md section 3.4.
   * Physical key codes keep them working on non-Latin keyboard layouts.
   */
  const runShortcut = (event: KeyboardEvent): boolean => {
    const command = event.ctrlKey || event.metaKey;

    if (command && !event.altKey && event.code === "KeyZ") {
      if (event.shiftKey) {
        interaction.redo();
      } else {
        interaction.undo();
      }
      return true;
    }
    if (command || event.altKey) {
      return false;
    }

    switch (event.key) {
      case "Escape":
        interaction.escape();
        return true;
      case "Enter":
        // Otherwise Enter belongs to whatever control has the focus.
        if (!interaction.status.routing) {
          return false;
        }
        interaction.finishRoute();
        return true;
      case "Delete":
      case "Backspace":
        interaction.deleteSelection();
        return true;
    }

    if (event.code === "KeyR" && !event.shiftKey) {
      // Holding R must not keep adding rotations to the history.
      if (!event.repeat) {
        interaction.rotate();
      }
      return true;
    }
    return false;
  };

  const onKey = (event: KeyboardEvent): void => {
    if (event.code === "ShiftLeft" || event.code === "ShiftRight") {
      // Shift only picks the bend direction of the next route legs.
      interaction.setVerticalFirst(event.type === "keydown");
      return;
    }

    if (event.code !== "Space") {
      if (
        event.type === "keydown" &&
        !isTextEntryTarget(event.target) &&
        runShortcut(event)
      ) {
        event.preventDefault();
        syncCursor();
      }
      return;
    }

    const typing = isTextEntryTarget(event.target);
    if (event.type === "keydown" && typing) {
      return;
    }

    interaction.setPanModifier(event.type === "keydown");
    // Only swallow Space (page scroll, button activation) over the canvas.
    if (!typing && interaction.pointerGridPoint !== null) {
      event.preventDefault();
    }
    syncCursor();
  };

  const onBlur = (): void => {
    interaction.setPanModifier(false);
    interaction.setVerticalFirst(false);
    syncCursor();
  };

  const surfaceListeners: [string, EventListener, AddEventListenerOptions?][] =
    [
      ["pointerdown", onPointerDown as EventListener],
      ["pointermove", onPointerMove as EventListener],
      ["pointerup", onPointerUp as EventListener],
      ["pointercancel", onPointerCancel as EventListener],
      ["lostpointercapture", onPointerCancel as EventListener],
      ["pointerleave", onPointerLeave],
      ["mousedown", onMouseDown as EventListener],
      ["wheel", onWheel as EventListener, { passive: false }],
    ];
  const keyboardListeners: [string, EventListener][] = [
    ["keydown", onKey as EventListener],
    ["keyup", onKey as EventListener],
    ["blur", onBlur],
  ];

  for (const [type, listener, options] of surfaceListeners) {
    surface.addEventListener(type, listener, options);
  }
  for (const [type, listener] of keyboardListeners) {
    keyboard.addEventListener(type, listener);
  }

  return {
    refreshCursor: syncCursor,
    dispose: () => {
      for (const [type, listener] of surfaceListeners) {
        surface.removeEventListener(type, listener);
      }
      for (const [type, listener] of keyboardListeners) {
        keyboard.removeEventListener(type, listener);
      }
      surface.style.cursor = "";
    },
  };
}
