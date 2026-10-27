import type { SymbolDefinition } from "./types";

/**
 * SPDT relay. The coil is a box on the left with its two pins on the left edge; the
 * small "+" above `coil_positive` marks the positive reference of the coil voltage and
 * current (the relay actuates on the magnitude, so the polarity does not matter to the
 * contacts). The contacts are on the right: `common` is the hinge, `normally_open` the
 * upper contact and `normally_closed` the lower one. The blade is drawn in the released
 * position, resting on the normally closed contact; the live state is shown in the
 * device panel, not in the drawing.
 */
export const relaySpdtSymbol: SymbolDefinition = {
  typeId: "relay_spdt",
  pins: [
    { id: "coil_positive", local: { x: -3, y: -1 } },
    { id: "coil_negative", local: { x: -3, y: 1 } },
    { id: "common", local: { x: 1, y: 0 } },
    { id: "normally_open", local: { x: 3, y: -1 } },
    { id: "normally_closed", local: { x: 3, y: 1 } },
  ],
  primitives: [
    // The coil box and its two leads.
    {
      kind: "polyline",
      points: [
        { x: -2, y: -0.6 },
        { x: -1, y: -0.6 },
        { x: -1, y: 0.6 },
        { x: -2, y: 0.6 },
        { x: -2, y: -0.6 },
      ],
    },
    {
      kind: "polyline",
      points: [
        { x: -3, y: -1 },
        { x: -1.5, y: -1 },
        { x: -1.5, y: -0.6 },
      ],
    },
    {
      kind: "polyline",
      points: [
        { x: -3, y: 1 },
        { x: -1.5, y: 1 },
        { x: -1.5, y: 0.6 },
      ],
    },
    // The positive mark of the coil.
    {
      kind: "polyline",
      points: [
        { x: -2.95, y: -1.4 },
        { x: -2.65, y: -1.4 },
      ],
    },
    {
      kind: "polyline",
      points: [
        { x: -2.8, y: -1.55 },
        { x: -2.8, y: -1.25 },
      ],
    },
    // The mechanical link from the coil to the blade, dashed.
    {
      kind: "polyline",
      points: [
        { x: -1, y: 0 },
        { x: -0.6, y: 0 },
      ],
    },
    {
      kind: "polyline",
      points: [
        { x: -0.3, y: 0 },
        { x: 0.1, y: 0 },
      ],
    },
    // The hinge, the blade at rest on the normally closed contact, and the two contacts.
    {
      kind: "polyline",
      points: [
        { x: 1, y: 0 },
        { x: 1.5, y: 0 },
      ],
    },
    {
      kind: "polyline",
      points: [
        { x: 1.5, y: 0 },
        { x: 2.45, y: 0.9 },
      ],
    },
    {
      kind: "polyline",
      points: [
        { x: 3, y: -1 },
        { x: 2.5, y: -1 },
      ],
    },
    {
      kind: "polyline",
      points: [
        { x: 3, y: 1 },
        { x: 2.5, y: 1 },
      ],
    },
    {
      kind: "circle",
      center: { x: 2.5, y: -1 },
      radius: 0.08,
    },
    {
      kind: "circle",
      center: { x: 2.5, y: 1 },
      radius: 0.08,
    },
  ],
};
