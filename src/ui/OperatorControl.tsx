import { useEffect, useRef } from "react";
import type { OperateAction } from "../editor/interaction";
import type { StableId } from "../generated/StableId";
import { HeldPress, pushButtonKey, switchKey } from "./operatorGesture";

export interface OperatorControlProps {
  readonly componentId: StableId;
  /** The part's display name, such as `S1`, which is also its accessible name. */
  readonly name: string;
  /** What the control operates, as words: "switch" or "push button". */
  readonly kind: "switch" | "pushButton";
  /** The position the Engine reported: closed (switch) or pressed (push button). */
  readonly actuated: boolean;
  /** True while there is no run on screen to send the position to. */
  readonly disabled: boolean;
  readonly onOperate: (componentId: StableId, action: OperateAction) => void;
}

/**
 * The keyboard and pointer way to operate a switch or a push button
 * (docs/contracts/stage-6-control-circuits.md section 6.3). It shows the position the
 * Engine last reported and nothing else: the control never keeps a position of its own.
 * A switch toggles on activation. A push button is pressed on pointer down or on a key
 * down of Space or Enter (a held key does not press it again), and is always released by
 * pointer up, leaving, cancel, key up, losing focus, and Escape.
 */
export function OperatorControl({
  componentId,
  name,
  kind,
  actuated,
  disabled,
  onOperate,
}: OperatorControlProps) {
  const gesture = useRef<HeldPress | null>(null);
  gesture.current ??= new HeldPress(() => {});
  gesture.current.setSender((action) => onOperate(componentId, action));
  const held = gesture.current;

  // A button must not stay pressed because its control went away.
  useEffect(() => () => held.release(), [held]);
  useEffect(() => {
    if (disabled) {
      held.forget();
    }
  }, [disabled, held]);

  if (kind === "switch") {
    return (
      <button
        type="button"
        role="switch"
        className="operator-control"
        aria-checked={actuated}
        aria-label={`${name} switch`}
        disabled={disabled}
        onClick={() => onOperate(componentId, "toggle")}
        onKeyDown={(event) => {
          const effect = switchKey("keydown", event.key, event.repeat);
          if (effect) event.preventDefault();
          if (effect?.toggle) onOperate(componentId, "toggle");
        }}
        onKeyUp={(event) => {
          if (switchKey("keyup", event.key, event.repeat)) {
            event.preventDefault();
          }
        }}
      >
        {actuated ? "Closed" : "Open"}
      </button>
    );
  }

  return (
    <button
      type="button"
      className="operator-control"
      aria-pressed={actuated}
      aria-label={`${name} push button`}
      disabled={disabled}
      onPointerDown={(event) => {
        if (event.button === 0) {
          event.currentTarget.setPointerCapture?.(event.pointerId);
          held.press();
        }
      }}
      onPointerUp={() => held.release()}
      onPointerCancel={() => held.release()}
      onLostPointerCapture={() => held.release()}
      onPointerLeave={() => held.release()}
      onBlur={() => held.release()}
      onKeyDown={(event) => {
        const effect = pushButtonKey("keydown", event.key, event.repeat);
        if (effect?.swallow) event.preventDefault();
        if (effect?.action === "press") held.press();
        if (effect?.action === "release") held.release();
      }}
      onKeyUp={(event) => {
        const effect = pushButtonKey("keyup", event.key, event.repeat);
        if (effect?.swallow) event.preventDefault();
        if (effect?.action === "release") held.release();
      }}
    >
      {actuated ? "Pressed" : "Hold to press"}
    </button>
  );
}
