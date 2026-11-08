import type { SymbolDefinition, SymbolPrimitive } from "./types";

const line = (...points: [number, number][]): SymbolPrimitive => ({
  kind: "polyline",
  points: points.map(([x, y]) => ({ x, y })),
});

/**
 * The common drawing of a bipolar transistor (IEC 60617): a base lead and bar on the left,
 * and the collector (above) and emitter (below) leaving the bar at an angle and then going
 * straight to their pins. The two types differ only in the arrow on the emitter, which
 * points out of the base for an NPN and into it for a PNP. Pin positions are those of
 * docs/contracts/stage-7-nonlinear-circuits.md section 8: `base` (-2, 0), `collector`
 * (1, -2), `emitter` (1, 2). The catalog lists the pins as collector, base, emitter.
 */
function bjtPrimitives(arrow: readonly SymbolPrimitive[]): SymbolPrimitive[] {
  return [
    // The base lead and the base bar.
    line([-2, 0], [-0.5, 0]),
    line([-0.5, -1], [-0.5, 1]),
    // The collector and the emitter, from the bar to their pins.
    line([-0.5, -0.4], [1, -1.4], [1, -2]),
    line([-0.5, 0.4], [1, 1.4], [1, 2]),
    ...arrow,
  ];
}

/** NPN: the arrow sits on the emitter and points away from the base. */
export const bjtNpnSymbol: SymbolDefinition = {
  typeId: "bjt_npn",
  pins: [
    { id: "collector", local: { x: 1, y: -2 } },
    { id: "base", local: { x: -2, y: 0 } },
    { id: "emitter", local: { x: 1, y: 2 } },
  ],
  primitives: bjtPrimitives([line([0.326, 1.131], [0.7, 1.2], [0.492, 0.881])]),
};

/** PNP: the arrow sits on the emitter and points toward the base. */
export const bjtPnpSymbol: SymbolDefinition = {
  typeId: "bjt_pnp",
  pins: [
    { id: "collector", local: { x: 1, y: -2 } },
    { id: "base", local: { x: -2, y: 0 } },
    { id: "emitter", local: { x: 1, y: 2 } },
  ],
  primitives: bjtPrimitives([
    line([0.233, 1.069], [0.025, 0.75], [0.399, 0.819]),
  ]),
};
