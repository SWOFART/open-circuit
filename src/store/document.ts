import {
  applyPatches,
  enablePatches,
  freeze,
  produceWithPatches,
  type Draft,
  type Patch,
} from "immer";
import type { ComponentCatalog } from "../generated/ComponentCatalog";
import type { ComponentSpec } from "../generated/ComponentSpec";
import type { GridPoint } from "../generated/GridPoint";
import type { JunctionSpec } from "../generated/JunctionSpec";
import type { ParameterDefinition } from "../generated/ParameterDefinition";
import type { ParameterId } from "../generated/ParameterId";
import type { ReferenceMarkerSpec } from "../generated/ReferenceMarkerSpec";
import type { Schematic } from "../generated/Schematic";
import type { StableId } from "../generated/StableId";
import type { WireSpec } from "../generated/WireSpec";
import { ComponentLabels } from "./labels";

enablePatches();

type WithoutId<T> = T extends { id: StableId } ? Omit<T, "id"> : never;

export type NewComponentSpec = WithoutId<ComponentSpec>;
export type NewWireSpec = WithoutId<WireSpec>;

export interface SchematicDocumentState {
  readonly schematic: Schematic;
  /**
   * Display names by stable ID. They stay with a component through edits and
   * undo/redo, and are not part of the saved schematic.
   */
  readonly labels: ReadonlyMap<StableId, string>;
  readonly canUndo: boolean;
  readonly canRedo: boolean;
  readonly isDirty: boolean;
}

interface HistoryEntry {
  readonly patches: Patch[];
  readonly inversePatches: Patch[];
}

export type StableIdFactory = () => StableId;

const UUID_PATTERN =
  /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/;
const VALID_ROTATIONS = new Set([0, 90, 180, 270]);

function createStableId(): StableId {
  if (
    typeof crypto === "undefined" ||
    typeof crypto.randomUUID !== "function"
  ) {
    throw new Error("This runtime cannot generate stable object IDs.");
  }

  return crypto.randomUUID();
}

function assertStableId(id: StableId): void {
  if (!UUID_PATTERN.test(id)) {
    throw new Error(`Object ID ${id} is not a canonical lowercase UUID.`);
  }
}

function assertPoint(point: GridPoint): void {
  if (Object.keys(point).length !== 2 || !("x" in point) || !("y" in point)) {
    throw new Error("A grid point must contain exactly x and y coordinates.");
  }
  for (const [axis, value] of [
    ["x", point.x],
    ["y", point.y],
  ] as const) {
    if (
      !Number.isInteger(value) ||
      value < -2_147_483_648 ||
      value > 2_147_483_647
    ) {
      throw new Error(
        `Grid coordinate ${axis} must be a signed 32-bit integer.`,
      );
    }
  }
}

function assertWirePoints(points: readonly GridPoint[]): void {
  if (points.length < 2) {
    throw new Error("A wire must contain at least two points.");
  }

  points.forEach((point, index) => {
    assertPoint(point);

    if (index === 0) {
      return;
    }

    const previous = points[index - 1];
    if (!previous) {
      throw new Error("A wire path contains an unreadable point.");
    }

    const sameX = previous.x === point.x;
    const sameY = previous.y === point.y;
    if (sameX === sameY) {
      throw new Error("Wire points must form distinct orthogonal segments.");
    }
  });
}

function allIds(schematic: Schematic): StableId[] {
  return [
    ...schematic.components.map((item) => item.id),
    ...schematic.wires.map((item) => item.id),
    ...schematic.junctions.map((item) => item.id),
    ...schematic.referenceMarkers.map((item) => item.id),
  ];
}

function assertUniqueIds(schematic: Schematic): void {
  const ids = allIds(schematic);
  const seen = new Set<StableId>();

  for (const id of ids) {
    assertStableId(id);
    if (seen.has(id)) {
      throw new Error(`Object ID ${id} is duplicated in the schematic.`);
    }
    seen.add(id);
  }
}

function assertSchematicShape(schematic: Schematic): void {
  if (
    schematic.schemaVersion !== 1 ||
    !Array.isArray(schematic.components) ||
    !Array.isArray(schematic.wires) ||
    !Array.isArray(schematic.junctions) ||
    !Array.isArray(schematic.referenceMarkers)
  ) {
    throw new Error("The document is not a version 1 schematic.");
  }

  assertUniqueIds(schematic);

  for (const component of schematic.components) {
    assertPoint(component.position);
    if (!VALID_ROTATIONS.has(component.rotationDeg)) {
      throw new Error(`Component ${component.id} has an invalid rotation.`);
    }
    if (
      Object.values(component.parameters).some(
        (value) => !Number.isFinite(value),
      )
    ) {
      throw new Error(`Component ${component.id} has a non-finite parameter.`);
    }
  }

  for (const wire of schematic.wires) {
    assertWirePoints(wire.points);
  }

  for (const marker of [
    ...schematic.junctions,
    ...schematic.referenceMarkers,
  ]) {
    assertPoint(marker.position);
  }
}

function matchesDomain(
  value: number,
  domain: ParameterDefinition["domain"],
): boolean {
  if (!Number.isFinite(value)) {
    return false;
  }

  switch (domain) {
    case "finite":
      return true;
    case "strictly_positive":
      return value > 0;
    case "non_negative":
      return value >= 0;
    case "zero_or_one":
      return value === 0 || value === 1;
  }
}

function assertComponentParameters(
  component: NewComponentSpec,
  catalog: ComponentCatalog,
): void {
  const entry = catalog.componentTypes.find(
    (candidate) => candidate.typeId === component.typeId,
  );
  if (!entry) {
    throw new Error(
      `Component type ${component.typeId} is not in the catalog.`,
    );
  }

  const parameterValues = component.parameters as Record<string, number>;
  const definitions = new Map(entry.parameters.map((item) => [item.id, item]));
  // Every required parameter is present, and nothing outside the catalog is.
  // An optional parameter, such as an initial condition, may be absent.
  if (
    Object.keys(parameterValues).some(
      (key) => !definitions.has(key as ParameterId),
    ) ||
    entry.parameters.some(
      (definition) =>
        !definition.optional && !(definition.id in parameterValues),
    )
  ) {
    throw new Error(
      `Component ${component.typeId} has an unexpected parameter set.`,
    );
  }

  for (const [key, value] of Object.entries(parameterValues)) {
    const definition = definitions.get(key as ParameterId);
    if (!definition || !matchesDomain(value, definition.domain)) {
      throw new Error(`Parameter ${key} is outside its catalog domain.`);
    }
  }
}

function sameSerializedDocument(left: Schematic, right: Schematic): boolean {
  return JSON.stringify(left) === JSON.stringify(right);
}

function pointCopy(point: GridPoint): GridPoint {
  return { x: point.x, y: point.y };
}

function pointsCopy(points: readonly GridPoint[]): GridPoint[] {
  return points.map(pointCopy);
}

/**
 * Owns only the editable schematic and its history. Engine results, diagnostics,
 * and frame state live in SimulationController.
 */
export class SchematicDocumentStore {
  private schematic: Schematic;
  private savedSchematic: Schematic;
  private readonly undoStack: HistoryEntry[] = [];
  private readonly redoStack: HistoryEntry[] = [];
  private readonly listeners = new Set<() => void>();
  private readonly issuedIds = new Set<StableId>();
  private readonly labels = new ComponentLabels();
  private state: SchematicDocumentState;

  constructor(
    schematic: Schematic,
    private readonly idFactory: StableIdFactory = createStableId,
  ) {
    assertSchematicShape(schematic);
    this.schematic = freeze(structuredClone(schematic), true) as Schematic;
    this.savedSchematic = this.schematic;
    allIds(this.schematic).forEach((id) => this.issuedIds.add(id));
    this.state = this.makeState();
  }

  readonly getState = (): SchematicDocumentState => this.state;

  readonly subscribe = (listener: () => void): (() => void) => {
    this.listeners.add(listener);
    return () => this.listeners.delete(listener);
  };

  addComponent(
    component: NewComponentSpec,
    catalog: ComponentCatalog,
  ): StableId {
    assertPoint(component.position);
    if (!VALID_ROTATIONS.has(component.rotationDeg)) {
      throw new Error(
        "A component rotation must be 0, 90, 180, or 270 degrees.",
      );
    }
    assertComponentParameters(component, catalog);

    const id = this.nextId();
    const created = {
      ...component,
      id,
      position: pointCopy(component.position),
      parameters: { ...component.parameters },
    } as ComponentSpec;
    this.commit((draft) => {
      draft.components.push(created);
    });
    return id;
  }

  removeComponent(id: StableId): void {
    this.commit((draft) => {
      const index = draft.components.findIndex((item) => item.id === id);
      if (index < 0) throw new Error(`Component ${id} does not exist.`);
      draft.components.splice(index, 1);
    });
  }

  moveComponent(id: StableId, position: GridPoint): void {
    assertPoint(position);
    this.commit((draft) => {
      const component = draft.components.find((item) => item.id === id);
      if (!component) throw new Error(`Component ${id} does not exist.`);
      component.position = pointCopy(position);
    });
  }

  rotateComponent(
    id: StableId,
    rotationDeg: ComponentSpec["rotationDeg"],
  ): void {
    if (!VALID_ROTATIONS.has(rotationDeg)) {
      throw new Error(
        "A component rotation must be 0, 90, 180, or 270 degrees.",
      );
    }
    this.commit((draft) => {
      const component = draft.components.find((item) => item.id === id);
      if (!component) throw new Error(`Component ${id} does not exist.`);
      component.rotationDeg = rotationDeg;
    });
  }

  /**
   * Moves and/or rotates a component and replaces the paths of the wires that
   * follow its pins, as one history transaction. The component keeps its ID.
   */
  transformComponent(
    id: StableId,
    position: GridPoint,
    rotationDeg: ComponentSpec["rotationDeg"],
    wires: readonly { id: StableId; points: readonly GridPoint[] }[] = [],
  ): void {
    assertPoint(position);
    if (!VALID_ROTATIONS.has(rotationDeg)) {
      throw new Error(
        "A component rotation must be 0, 90, 180, or 270 degrees.",
      );
    }
    wires.forEach((wire) => assertWirePoints(wire.points));

    this.commit((draft) => {
      const component = draft.components.find((item) => item.id === id);
      if (!component) throw new Error(`Component ${id} does not exist.`);
      component.position = pointCopy(position);
      component.rotationDeg = rotationDeg;

      for (const edit of wires) {
        const wire = draft.wires.find((item) => item.id === edit.id);
        if (!wire) throw new Error(`Wire ${edit.id} does not exist.`);
        wire.points = pointsCopy(edit.points);
      }
    });
  }

  setParameter(
    componentId: StableId,
    parameterId: ParameterId,
    value: number,
    catalog: ComponentCatalog,
  ): void {
    const currentComponent = this.schematic.components.find(
      (item) => item.id === componentId,
    );
    if (!currentComponent)
      throw new Error(`Component ${componentId} does not exist.`);
    const definition = catalog.componentTypes
      .find((entry) => entry.typeId === currentComponent.typeId)
      ?.parameters.find((item) => item.id === parameterId);
    if (!definition || !matchesDomain(value, definition.domain)) {
      throw new Error(
        `Parameter ${parameterId} is outside its catalog domain.`,
      );
    }

    this.commit((draft) => {
      const component = draft.components.find(
        (item) => item.id === componentId,
      );
      if (!component)
        throw new Error(`Component ${componentId} does not exist.`);

      // The catalog lookup above proved that this type has the parameter.
      (component.parameters as Record<string, number>)[parameterId] = value;
    });
  }

  /**
   * Removes an optional parameter, for example an initial condition, so that
   * the component starts from the operating point again.
   */
  clearParameter(
    componentId: StableId,
    parameterId: ParameterId,
    catalog: ComponentCatalog,
  ): void {
    const currentComponent = this.schematic.components.find(
      (item) => item.id === componentId,
    );
    if (!currentComponent)
      throw new Error(`Component ${componentId} does not exist.`);
    const definition = catalog.componentTypes
      .find((entry) => entry.typeId === currentComponent.typeId)
      ?.parameters.find((item) => item.id === parameterId);
    if (!definition?.optional) {
      throw new Error(`Parameter ${parameterId} cannot be cleared.`);
    }

    this.commit((draft) => {
      const component = draft.components.find(
        (item) => item.id === componentId,
      );
      if (!component)
        throw new Error(`Component ${componentId} does not exist.`);

      delete (component.parameters as Record<string, number | undefined>)[
        parameterId
      ];
    });
  }

  addWire(wire: NewWireSpec): StableId {
    assertWirePoints(wire.points);
    const id = this.nextId();
    const created: WireSpec = { id, points: pointsCopy(wire.points) };
    this.commit((draft) => {
      draft.wires.push(created);
    });
    return id;
  }

  editWire(id: StableId, points: readonly GridPoint[]): void {
    assertWirePoints(points);
    this.commit((draft) => {
      const wire = draft.wires.find((item) => item.id === id);
      if (!wire) throw new Error(`Wire ${id} does not exist.`);
      wire.points = pointsCopy(points);
    });
  }

  removeWire(id: StableId): void {
    this.commit((draft) => {
      const index = draft.wires.findIndex((item) => item.id === id);
      if (index < 0) throw new Error(`Wire ${id} does not exist.`);
      draft.wires.splice(index, 1);
    });
  }

  addJunction(position: GridPoint): StableId {
    return this.addMarker("junctions", position);
  }

  moveJunction(id: StableId, position: GridPoint): void {
    this.moveMarker("junctions", id, position);
  }

  removeJunction(id: StableId): void {
    this.removeMarker("junctions", id);
  }

  addReferenceMarker(position: GridPoint): StableId {
    return this.addMarker("referenceMarkers", position);
  }

  moveReferenceMarker(id: StableId, position: GridPoint): void {
    this.moveMarker("referenceMarkers", id, position);
  }

  removeReferenceMarker(id: StableId): void {
    this.removeMarker("referenceMarkers", id);
  }

  undo(): boolean {
    const entry = this.undoStack.pop();
    if (!entry) return false;
    this.schematic = applyPatches(this.schematic, entry.inversePatches);
    this.redoStack.push(entry);
    this.publish();
    return true;
  }

  redo(): boolean {
    const entry = this.redoStack.pop();
    if (!entry) return false;
    this.schematic = applyPatches(this.schematic, entry.patches);
    this.undoStack.push(entry);
    this.publish();
    return true;
  }

  /** Records exactly the persisted snapshot as the last successfully saved state. */
  markSaved(schematic: Schematic = this.schematic): void {
    assertSchematicShape(schematic);
    this.savedSchematic = freeze(structuredClone(schematic), true) as Schematic;
    this.publish();
  }

  /** Replaces the open document and starts a fresh, clean history. */
  replaceDocument(schematic: Schematic): void {
    assertSchematicShape(schematic);
    this.schematic = freeze(structuredClone(schematic), true) as Schematic;
    this.savedSchematic = this.schematic;
    this.undoStack.length = 0;
    this.redoStack.length = 0;
    allIds(this.schematic).forEach((id) => this.issuedIds.add(id));
    // A different document is numbered from scratch, in file order.
    this.labels.reset();
    this.publish();
  }

  private addMarker(
    collection: "junctions" | "referenceMarkers",
    position: GridPoint,
  ): StableId {
    assertPoint(position);
    const id = this.nextId();
    this.commit((draft) => {
      const marker = { id, position: pointCopy(position) };
      (draft[collection] as Draft<(JunctionSpec | ReferenceMarkerSpec)[]>).push(
        marker,
      );
    });
    return id;
  }

  private moveMarker(
    collection: "junctions" | "referenceMarkers",
    id: StableId,
    position: GridPoint,
  ): void {
    assertPoint(position);
    this.commit((draft) => {
      const marker = draft[collection].find((item) => item.id === id);
      if (!marker) throw new Error(`Marker ${id} does not exist.`);
      marker.position = pointCopy(position);
    });
  }

  private removeMarker(
    collection: "junctions" | "referenceMarkers",
    id: StableId,
  ): void {
    this.commit((draft) => {
      const index = draft[collection].findIndex((item) => item.id === id);
      if (index < 0) throw new Error(`Marker ${id} does not exist.`);
      draft[collection].splice(index, 1);
    });
  }

  private nextId(): StableId {
    const id = this.idFactory();
    assertStableId(id);
    if (this.issuedIds.has(id)) {
      throw new Error(
        `Object ID ${id} has already been used in this document store.`,
      );
    }
    this.issuedIds.add(id);
    return id;
  }

  private commit(recipe: (draft: Draft<Schematic>) => void): void {
    const [next, patches, inversePatches] = produceWithPatches(
      this.schematic,
      recipe,
    );
    if (patches.length === 0) return;
    this.schematic = next;
    this.undoStack.push({ patches, inversePatches });
    this.redoStack.length = 0;
    this.publish();
  }

  private makeState(): SchematicDocumentState {
    return Object.freeze({
      schematic: this.schematic,
      labels: this.labels.labelsFor(this.schematic),
      canUndo: this.undoStack.length > 0,
      canRedo: this.redoStack.length > 0,
      isDirty: !sameSerializedDocument(this.schematic, this.savedSchematic),
    });
  }

  private publish(): void {
    this.state = this.makeState();
    for (const listener of [...this.listeners]) listener();
  }
}
