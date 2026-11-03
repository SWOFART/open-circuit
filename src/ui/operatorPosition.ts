import type { OperateAction } from "../editor/interaction";
import type { DeviceState } from "../generated/DeviceState";
import type { OperatorPosition } from "../generated/OperatorPosition";

/**
 * The position to request for a user action, from the state the Engine last reported for
 * the device. A switch toggles to the other position; a push button is pressed and
 * released. It is null when there is no state to go by, or the device is not the kind the
 * action is for: the interface then sends nothing instead of guessing.
 */
export function operatorPosition(
  action: OperateAction,
  state: DeviceState | undefined,
): OperatorPosition | null {
  switch (action) {
    case "toggle":
      return state?.kind === "switch"
        ? state.position === "closed"
          ? "open"
          : "closed"
        : null;
    case "press":
      return state?.kind === "pushButton" ? "pressed" : null;
    case "release":
      return state?.kind === "pushButton" ? "released" : null;
  }
}
