import type { SymbolDefinition, SymbolPrimitive } from "./types";

function line(points: readonly (readonly [number, number])[]): SymbolPrimitive {
  return { kind: "polyline", points: points.map(([x, y]) => ({ x, y })) };
}

function contact(
  typeId: "limit_switch" | "emergency_stop" | "key_switch",
): SymbolDefinition {
  const control = {
    limit_switch: line([
      [-0.45, -0.65],
      [0, -0.9],
      [0.45, -0.65],
    ]),
    emergency_stop: line([
      [-0.55, -0.55],
      [-0.4, -0.85],
      [0.4, -0.85],
      [0.55, -0.55],
      [-0.55, -0.55],
    ]),
    key_switch: line([
      [-0.25, -0.8],
      [0.45, -0.8],
      [0.45, -0.6],
    ]),
  }[typeId];
  const drawn: NonNullable<SymbolDefinition["drawn"]> = (state) => {
    const closed =
      state?.kind === "limitSwitch"
        ? state.position === "pressed"
        : state?.kind === "emergencyStop"
          ? state.position === "armed"
          : state?.kind === "keySwitch"
            ? state.position === "on"
            : typeId === "emergency_stop";
    return [
      line([
        [-1, 0],
        [-0.45, 0],
        [0.45, closed ? 0 : -0.4],
      ]),
      line([
        [0.45, 0],
        [1, 0],
      ]),
      control,
    ];
  };
  return {
    typeId,
    pins: [
      { id: "p", local: { x: -1, y: 0 } },
      { id: "n", local: { x: 1, y: 0 } },
    ],
    operatingArea: { center: { x: 0, y: -0.7 }, radius: 0.55 },
    primitives: drawn(undefined),
    drawn,
  };
}

export const limitSwitchSymbol = contact("limit_switch");
export const emergencyStopSymbol = contact("emergency_stop");
export const keySwitchSymbol = contact("key_switch");

const drawSelector: NonNullable<SymbolDefinition["drawn"]> = (state) => {
  const position = state?.kind === "selectorSwitch" ? state.position : "center";
  return [
    line([
      [-2, 0],
      [-1, 0],
      [1, position === "left" ? -1 : position === "right" ? 1 : 0],
    ]),
    line([
      [1, -1],
      [2, -1],
    ]),
    line([
      [1, 1],
      [2, 1],
    ]),
  ];
};
export const selectorSwitchSymbol: SymbolDefinition = {
  typeId: "selector_switch",
  pins: [
    { id: "com", local: { x: -2, y: 0 } },
    { id: "throw1", local: { x: 2, y: -1 } },
    { id: "throw2", local: { x: 2, y: 1 } },
  ],
  primitives: drawSelector(undefined),
  drawn: drawSelector,
};
