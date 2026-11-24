import type { SymbolDefinition, SymbolPrimitive } from "./types";

function line(points: readonly (readonly [number, number])[]): SymbolPrimitive {
  return { kind: "polyline", points: points.map(([x, y]) => ({ x, y })) };
}

function symbol(
  typeId: "relay_on_delay" | "relay_off_delay",
): SymbolDefinition {
  function drawn(
    state: Parameters<NonNullable<SymbolDefinition["drawn"]>>[0],
  ): readonly SymbolPrimitive[] {
    const picked =
      (state?.kind === "relayOnDelay" || state?.kind === "relayOffDelay") &&
      state.outputOn;
    return [
      line([
        [-3, -2],
        [-1, -2],
      ]),
      line([
        [1, -2],
        [3, -2],
      ]),
      line([
        [-1, -2.5],
        [1, -2.5],
        [1, -1.5],
        [-1, -1.5],
        [-1, -2.5],
      ]),
      line([
        [-0.35, -2.3],
        [0, -2.3],
        [0, -1.75],
        [typeId === "relay_on_delay" ? 0.5 : -0.5, -1.75],
      ]),
      line([
        [-3, 0],
        [-0.75, 0],
        [0.75, picked ? 0 : -0.5],
      ]),
      line([
        [0.75, 0],
        [3, 0],
      ]),
      line([
        [-3, 2],
        [-0.75, 2],
        [0.75, picked ? 1.5 : 2],
      ]),
      line([
        [0.75, 2],
        [3, 2],
      ]),
    ];
  }
  return {
    typeId,
    pins: [
      { id: "a1", local: { x: -3, y: -2 } },
      { id: "a2", local: { x: 3, y: -2 } },
      { id: "no1", local: { x: -3, y: 0 } },
      { id: "no2", local: { x: 3, y: 0 } },
      { id: "nc1", local: { x: -3, y: 2 } },
      { id: "nc2", local: { x: 3, y: 2 } },
    ],
    primitives: drawn(undefined),
    drawn,
  };
}

export const onDelayRelaySymbol = symbol("relay_on_delay");
export const offDelayRelaySymbol = symbol("relay_off_delay");
