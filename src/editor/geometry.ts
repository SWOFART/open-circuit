import type { ComponentSpec } from "../generated/ComponentSpec";
import type { GeometryEnvelope } from "../generated/GeometryEnvelope";
import type { GridPoint } from "../generated/GridPoint";
import type { ResolvedPinPosition } from "../generated/ResolvedPinPosition";
import type { Schematic } from "../generated/Schematic";
import type { LocalPoint, SymbolRegistry } from "../symbols";

export type Rotation = ComponentSpec["rotationDeg"];

// `n + 0` turns -0 into +0 so results compare equal to plain integers.
function normalizeZero(value: number): number {
  return value + 0;
}

/**
 * Rotates a local vector clockwise in screen coordinates (x right, y down),
 * exactly as specified in docs/contracts/stage-1.md section 2.1.
 */
export function rotateLocal(point: LocalPoint, rotation: Rotation): LocalPoint {
  const { x: u, y: v } = point;

  switch (rotation) {
    case 0:
      return { x: normalizeZero(u), y: normalizeZero(v) };
    case 90:
      return { x: normalizeZero(-v), y: normalizeZero(u) };
    case 180:
      return { x: normalizeZero(-u), y: normalizeZero(-v) };
    case 270:
      return { x: normalizeZero(v), y: normalizeZero(-u) };
  }
}

/** Maps a symbol-local point to world coordinates for a placed component. */
export function toWorld(
  local: LocalPoint,
  component: Pick<ComponentSpec, "position" | "rotationDeg">,
): LocalPoint {
  const rotated = rotateLocal(local, component.rotationDeg);

  return {
    x: normalizeZero(rotated.x + component.position.x),
    y: normalizeZero(rotated.y + component.position.y),
  };
}

function toGridPoint(point: LocalPoint, description: string): GridPoint {
  if (!Number.isInteger(point.x) || !Number.isInteger(point.y)) {
    throw new Error(`${description} does not land on an integer grid point.`);
  }

  return { x: point.x, y: point.y };
}

/** World-grid pin positions the compile request must carry for a component. */
export function resolvePinPositions(
  component: ComponentSpec,
  registry: SymbolRegistry,
): ResolvedPinPosition[] {
  const symbol = registry[component.typeId];

  return symbol.pins.map((pin) => ({
    pinId: pin.id,
    position: toGridPoint(
      toWorld(pin.local, component),
      `Pin ${pin.id} of component ${component.id}`,
    ),
  }));
}

export function buildGeometryEnvelope(
  schematic: Schematic,
  registry: SymbolRegistry,
): GeometryEnvelope {
  return {
    components: schematic.components.map((component) => ({
      componentId: component.id,
      pins: resolvePinPositions(component, registry),
    })),
  };
}
