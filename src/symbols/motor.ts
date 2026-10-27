import type { SymbolDefinition } from "./types";

/**
 * Permanent-magnet DC motor: a circle with an "M". The small "+" above `pin1` marks the
 * positive armature terminal: positive current into `pin1` gives positive torque and
 * positive rotation.
 */
export const pmdcMotorSymbol: SymbolDefinition = {
  typeId: "pmdc_motor",
  pins: [
    { id: "pin1", local: { x: -1, y: 0 } },
    { id: "pin2", local: { x: 1, y: 0 } },
  ],
  primitives: [
    {
      kind: "circle",
      center: { x: 0, y: 0 },
      radius: 0.7,
    },
    {
      kind: "polyline",
      points: [
        { x: -1, y: 0 },
        { x: -0.7, y: 0 },
      ],
    },
    {
      kind: "polyline",
      points: [
        { x: 0.7, y: 0 },
        { x: 1, y: 0 },
      ],
    },
    // The letter M.
    {
      kind: "polyline",
      points: [
        { x: -0.3, y: 0.3 },
        { x: -0.3, y: -0.3 },
        { x: 0, y: 0.05 },
        { x: 0.3, y: -0.3 },
        { x: 0.3, y: 0.3 },
      ],
    },
    // The positive mark of pin1.
    {
      kind: "polyline",
      points: [
        { x: -0.95, y: -0.4 },
        { x: -0.65, y: -0.4 },
      ],
    },
    {
      kind: "polyline",
      points: [
        { x: -0.8, y: -0.55 },
        { x: -0.8, y: -0.25 },
      ],
    },
  ],
};
