import type { DeviceState } from "../generated/DeviceState";
import type { SymbolDefinition, SymbolPrimitive } from "./types";

const line = (...points: [number, number][]): SymbolPrimitive => ({
  kind: "polyline",
  points: points.map(([x, y]) => ({ x, y })),
});

/**
 * A push button (IEC 60617): two contacts, a bridge, and a push actuator above it.
 * The normally closed button has a bar under the contact. The bridge lies on the contacts
 * when the Engine says the button conducts, and is lifted when it does not; the actuator
 * is drawn pushed in while the button is pressed.
 */
function pushButtonPrimitives(
  normallyClosed: boolean,
  state: DeviceState | undefined,
): readonly SymbolPrimitive[] {
  const pressed = state?.kind === "pushButton" && state.position === "pressed";
  const conducting =
    state?.kind === "pushButton" ? state.conducting : normallyClosed;
  const bridgeY = conducting ? 0 : -0.4;
  const capY = pressed ? -0.65 : -1;

  return [
    line([-1, 0], [-0.5, 0]),
    line([0.5, 0], [1, 0]),
    { kind: "circle", center: { x: -0.5, y: 0 }, radius: 0.08 },
    { kind: "circle", center: { x: 0.5, y: 0 }, radius: 0.08 },
    line([-0.5, bridgeY], [0.5, bridgeY]),
    // The actuator: a stem from the bridge to a cap.
    line([0, bridgeY], [0, capY]),
    line([-0.3, capY], [0.3, capY]),
    ...(normallyClosed ? [line([-0.3, 0.35], [0.3, 0.35])] : []),
  ];
}

const operatingArea = { center: { x: 0, y: -0.7 }, radius: 0.6 };

/** Normally open push button: it conducts only while it is pressed. */
export const pushButtonNoSymbol: SymbolDefinition = {
  typeId: "push_button_no",
  pins: [
    { id: "pin1", local: { x: -1, y: 0 } },
    { id: "pin2", local: { x: 1, y: 0 } },
  ],
  primitives: pushButtonPrimitives(false, undefined),
  drawn: (state) => pushButtonPrimitives(false, state),
  operatingArea,
};

/** Normally closed push button: it conducts except while it is pressed. */
export const pushButtonNcSymbol: SymbolDefinition = {
  typeId: "push_button_nc",
  pins: [
    { id: "pin1", local: { x: -1, y: 0 } },
    { id: "pin2", local: { x: 1, y: 0 } },
  ],
  primitives: pushButtonPrimitives(true, undefined),
  drawn: (state) => pushButtonPrimitives(true, state),
  operatingArea,
};
