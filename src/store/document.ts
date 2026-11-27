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
import type { SheetSpec } from "../generated/SheetSpec";
import type { StableId } from "../generated/StableId";
import type { TitleBlock } from "../generated/TitleBlock";
import type { ClipboardSchematic } from "../editor/clipboard";
import { sheetsOverlap } from "../editor/sheetRules";
import type { WireSpec } from "../generated/WireSpec";
import type { WireNumberEntry } from "../generated/WireNumberEntry";
import {
  MAX_RAILS,
  nextRailName,
  railExtentProblem,
  railNameProblem,
} from "../editor/railRules";
import {
  DESIGNATOR_MESSAGES,
  designatorProblem,
  nextDesignator,
} from "../editor/designators";
import { MAX_NOTES, NOTE_MESSAGES, noteProblem } from "../editor/noteRules";
import {
  MAX_TERMINALS,
  pointAtWireOffset,
  samePoint,
  validTerminalName,
  validWireNumber,
  wireOffset,
} from "../editor/terminalRules";
import { resolvePinPositions } from "../editor/geometry";
import { symbols } from "../symbols";
import { ComponentLabels } from "./labels";

enablePatches();

type WithoutId<T> = T extends { id: StableId } ? Omit<T, "id"> : never;

export type NewComponentSpec = WithoutId<ComponentSpec>;
export type NewWireSpec = WithoutId<WireSpec>;

/** A rail to add: the name is optional, and the smallest unused `RAIL<n>` fills it in. */
export interface NewRailSpec {
  readonly name?: string;
  readonly x: number;
  readonly top: number;
  readonly bottom: number;
}

/** What a rail edit changes; whatever is left out stays. */
export interface RailPatch {
  readonly name?: string;
  readonly x?: number;
  readonly top?: number;
  readonly bottom?: number;
}

function assertRailName(name: string, others: readonly string[]): void {
  switch (railNameProblem(name, others)) {
    case "empty":
      throw new Error("A rail needs a name.");
    case "grammar":
      throw new Error(
        "A rail name is 1 to 32 letters, digits, underscores, plus signs or hyphens, and starts with a letter or digit.",
      );
    case "duplicate":
      throw new Error(`Another rail is already named ${name}.`);
    case null:
      return;
  }
}

function assertRailExtent(x: number, top: number, bottom: number): void {
  switch (railExtentProblem(x, top, bottom)) {
    case "coordinate":
      throw new Error("A rail coordinate must be a whole number on the grid.");
    case "extent":
      throw new Error(
        "A rail must be between 2 and 1,000,000 grid units long.",
      );
    case null:
      return;
  }
}

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

/** The optional mirror flag is a boolean when it is there (stage 16 contract, section 5.2). */
function assertMirrorFlag(component: { readonly mirrored?: unknown }): void {
  if (
    component.mirrored !== undefined &&
    typeof component.mirrored !== "boolean"
  ) {
    throw new Error("A component mirror flag must be true or false.");
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
    ...schematic.rails.map((item) => item.id),
    ...schematic.documentation.terminals.map((item) => item.id),
    ...schematic.documentation.notes.map((item) => item.id),
    ...schematic.documentation.sheets.map((item) => item.id),
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
    schematic.schemaVersion !== 3 ||
    !Array.isArray(schematic.components) ||
    !Array.isArray(schematic.wires) ||
    !Array.isArray(schematic.junctions) ||
    !Array.isArray(schematic.referenceMarkers) ||
    !Array.isArray(schematic.rails) ||
    typeof schematic.documentation !== "object" ||
    schematic.documentation === null
  ) {
    throw new Error("The document is not a version 3 schematic.");
  }

  assertUniqueIds(schematic);

  for (const component of schematic.components) {
    assertMirrorFlag(component);
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
    case "zero_one_or_two":
      return value === 0 || value === 1 || value === 2;
    case "integer":
      return Number.isInteger(value);
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
  // While a transaction runs, its edits collect here and become one history entry.
  private batch: {
    patches: Patch[];
    inversePatches: Patch[];
    topology: boolean;
  } | null = null;
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

  /**
   * Adds a component. With `designate` the same history entry stores its designator: the
   * catalog's default letters and the next number (contract section 3.4). Assignments hold no
   * documentation, so the caller leaves it off there.
   */
  addComponent(
    component: NewComponentSpec,
    catalog: ComponentCatalog,
    options: { readonly designate?: boolean } = {},
  ): StableId {
    assertPoint(component.position);
    if (!VALID_ROTATIONS.has(component.rotationDeg)) {
      throw new Error(
        "A component rotation must be 0, 90, 180, or 270 degrees.",
      );
    }
    assertComponentParameters(component, catalog);
    assertMirrorFlag(component);

    const id = this.nextId();
    const created = {
      ...component,
      id,
      position: pointCopy(component.position),
      parameters: { ...component.parameters },
    } as ComponentSpec;
    if (created.mirrored !== true) delete created.mirrored;
    const designator = options.designate
      ? nextDesignator(
          catalog.componentTypes.find(
            (entry) => entry.typeId === created.typeId,
          )?.designatorPrefix ?? "",
          this.schematic.documentation.designators.map(
            (entry) => entry.designator,
          ),
        )
      : null;
    this.commit((draft) => {
      draft.components.push(created);
      if (designator !== null) {
        draft.documentation.designators.push({
          componentId: id,
          designator,
        });
      }
    });
    return id;
  }

  /** The component's stored designator, if it has one. */
  designatorOf(id: StableId): string | undefined {
    return this.schematic.documentation.designators.find(
      (entry) => entry.componentId === id,
    )?.designator;
  }

  /**
   * Names a component. One history entry; refused, with the document unchanged, when the text
   * is not a designator or another part has it. It is a documentation edit: nothing electrical
   * changes.
   */
  setDesignator(id: StableId, text: string): void {
    if (!this.schematic.components.some((item) => item.id === id)) {
      throw new Error(`Component ${id} does not exist.`);
    }
    const problem = designatorProblem(
      text,
      this.schematic.documentation.designators
        .filter((entry) => entry.componentId !== id)
        .map((entry) => entry.designator),
    );
    if (problem !== null) {
      throw new Error(DESIGNATOR_MESSAGES[problem]);
    }
    this.commit((draft) => {
      const entry = draft.documentation.designators.find(
        (item) => item.componentId === id,
      );
      if (entry) {
        entry.designator = text;
      } else {
        draft.documentation.designators.push({
          componentId: id,
          designator: text,
        });
      }
    });
  }

  /**
   * Places a text note: documentation, not part of the circuit. One history entry; refused,
   * with the document unchanged, for a text outside the rules of contract section 6 or when the
   * document already holds the most notes.
   */
  addNote(position: GridPoint, text: string): StableId {
    assertPoint(position);
    this.assertNoteText(text);
    if (this.schematic.documentation.notes.length >= MAX_NOTES) {
      throw new Error(`A document holds at most ${MAX_NOTES} notes.`);
    }
    const id = this.nextId();
    this.commit((draft) => {
      draft.documentation.notes.push({
        id,
        position: pointCopy(position),
        text,
      });
    });
    return id;
  }

  moveNote(id: StableId, position: GridPoint): void {
    assertPoint(position);
    this.commit((draft) => {
      const note = draft.documentation.notes.find((item) => item.id === id);
      if (!note) throw new Error(`Note ${id} does not exist.`);
      note.position = pointCopy(position);
    });
  }

  setNoteText(id: StableId, text: string): void {
    this.assertNoteText(text);
    this.commit((draft) => {
      const note = draft.documentation.notes.find((item) => item.id === id);
      if (!note) throw new Error(`Note ${id} does not exist.`);
      note.text = text;
    });
  }

  removeNote(id: StableId): void {
    this.commit((draft) => {
      const index = draft.documentation.notes.findIndex(
        (item) => item.id === id,
      );
      if (index < 0) throw new Error(`Note ${id} does not exist.`);
      draft.documentation.notes.splice(index, 1);
    });
  }

  private assertNoteText(text: string): void {
    const problem = noteProblem(text);
    if (problem !== null) throw new Error(NOTE_MESSAGES[problem]);
  }

  addTerminal(position: GridPoint, strip: string, label: string): StableId {
    assertPoint(position);
    if (this.schematic.documentation.terminals.length >= MAX_TERMINALS) {
      throw new Error(`A document holds at most ${MAX_TERMINALS} terminals.`);
    }
    if (!this.terminalAttachment(this.schematic, position)) {
      throw new Error("Place the terminal on a pin or a wire.");
    }
    if (
      this.schematic.documentation.terminals.some((terminal) =>
        samePoint(terminal.position, position),
      )
    ) {
      throw new Error("This point already has a terminal.");
    }
    this.assertTerminalName(strip, label);
    const id = this.nextId();
    this.commit((draft) => {
      draft.documentation.terminals.push({
        id,
        strip,
        label,
        position: pointCopy(position),
      });
    });
    return id;
  }

  setTerminalName(id: StableId, strip: string, label: string): void {
    this.assertTerminalName(strip, label, id);
    this.commit((draft) => {
      const terminal = draft.documentation.terminals.find(
        (entry) => entry.id === id,
      );
      if (!terminal) throw new Error("This terminal no longer exists.");
      terminal.strip = strip;
      terminal.label = label;
    });
  }

  removeTerminal(id: StableId): void {
    this.commit((draft) => {
      draft.documentation.terminals = draft.documentation.terminals.filter(
        (terminal) => terminal.id !== id,
      );
    });
  }

  private assertTerminalName(
    strip: string,
    label: string,
    id?: StableId,
  ): void {
    if (!validTerminalName(strip, label)) {
      throw new Error(
        "Enter a strip such as X1 and a label of 1 to 8 letters, digits, or + . _ -.",
      );
    }
    if (
      this.schematic.documentation.terminals.some(
        (entry) =>
          entry.id !== id && entry.strip === strip && entry.label === label,
      )
    ) {
      throw new Error("That strip and label are already in use.");
    }
  }

  private terminalAttachment(schematic: Schematic, point: GridPoint) {
    for (const component of schematic.components) {
      if (!symbols[component.typeId]) continue;
      const pin = resolvePinPositions(component, symbols).find((pin) =>
        samePoint(pin.position, point),
      );
      if (pin)
        return { kind: "pin" as const, id: component.id, pinId: pin.pinId };
    }
    for (const wire of schematic.wires) {
      const offset = wireOffset(wire.points, point);
      if (offset !== null)
        return { kind: "wire" as const, id: wire.id, offset };
    }
    return null;
  }

  /**
   * Re-attaches every documentation terminal to the pin or wire it sat on in `before`, in the
   * drawing `draft` now holds. Throws when two terminals would share a point.
   */
  private followTerminals(draft: Draft<Schematic>, before: Schematic): void {
    if (draft.documentation.terminals.length === 0) return;
    const occupied = new Set<string>();
    draft.documentation.terminals = draft.documentation.terminals.filter(
      (terminal) => {
        const attachment = this.terminalAttachment(before, terminal.position);
        let position: GridPoint | undefined = terminal.position;
        if (attachment?.kind === "pin") {
          const component = draft.components.find(
            (entry) => entry.id === attachment.id,
          );
          position =
            component &&
            resolvePinPositions(component, symbols).find(
              (pin) => pin.pinId === attachment.pinId,
            )?.position;
        } else if (attachment?.kind === "wire") {
          const wire = draft.wires.find((entry) => entry.id === attachment.id);
          position = wire && pointAtWireOffset(wire.points, attachment.offset);
        }
        if (!position) return false;
        const key = `${position.x},${position.y}`;
        if (occupied.has(key))
          throw new Error("This move would put two terminals at one point.");
        occupied.add(key);
        if (!samePoint(position, terminal.position))
          terminal.position = position;
        return true;
      },
    );
  }

  private commitTopology(recipe: (draft: Draft<Schematic>) => void): void {
    if (this.batch) {
      // Inside a transaction the terminals follow once, after the last edit, so an
      // intermediate drawing never raises a collision the finished one does not have.
      this.batch.topology = true;
      this.commit(recipe);
      return;
    }
    this.commit((draft) => {
      recipe(draft);
      this.followTerminals(draft, this.schematic);
    });
  }

  removeComponent(id: StableId): void {
    this.commitTopology((draft) => {
      const index = draft.components.findIndex((item) => item.id === id);
      if (index < 0) throw new Error(`Component ${id} does not exist.`);
      draft.components.splice(index, 1);
      // The name goes with the part, in the same history entry.
      const named = draft.documentation.designators.findIndex(
        (entry) => entry.componentId === id,
      );
      if (named >= 0) draft.documentation.designators.splice(named, 1);
    });
  }

  moveComponent(id: StableId, position: GridPoint): void {
    assertPoint(position);
    this.commitTopology((draft) => {
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
    this.commitTopology((draft) => {
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
    mirrored?: boolean,
  ): void {
    assertPoint(position);
    if (mirrored !== undefined && typeof mirrored !== "boolean") {
      throw new Error("A component mirror flag must be true or false.");
    }
    if (!VALID_ROTATIONS.has(rotationDeg)) {
      throw new Error(
        "A component rotation must be 0, 90, 180, or 270 degrees.",
      );
    }
    wires.forEach((wire) => assertWirePoints(wire.points));

    this.commitTopology((draft) => {
      const component = draft.components.find((item) => item.id === id);
      if (!component) throw new Error(`Component ${id} does not exist.`);
      component.position = pointCopy(position);
      component.rotationDeg = rotationDeg;
      if (mirrored === true) {
        component.mirrored = true;
      } else if (mirrored === false) {
        // An unmirrored part has no field, so an unmirrored file is as it always was.
        delete component.mirrored;
      }

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
    this.commitTopology((draft) => {
      const wire = draft.wires.find((item) => item.id === id);
      if (!wire) throw new Error(`Wire ${id} does not exist.`);
      wire.points = pointsCopy(points);
    });
  }

  removeWire(id: StableId): void {
    this.commitTopology((draft) => {
      const index = draft.wires.findIndex((item) => item.id === id);
      if (index < 0) throw new Error(`Wire ${id} does not exist.`);
      draft.wires.splice(index, 1);
      draft.documentation.wireNumbers = draft.documentation.wireNumbers.filter(
        (entry) => entry.wireId !== id,
      );
    });
  }

  /** Set a drawn label, or clear it when the field is empty. */
  setWireNumber(id: StableId, number: string): void {
    if (number !== "" && !validWireNumber(number)) {
      throw new Error(
        "A wire number must contain 1 to 16 letters, digits, or + . _ -.",
      );
    }
    this.commit((draft) => {
      if (!draft.wires.some((wire) => wire.id === id)) {
        throw new Error(`Wire ${id} does not exist.`);
      }
      const entries = draft.documentation.wireNumbers;
      const index = entries.findIndex((entry) => entry.wireId === id);
      if (index >= 0) {
        if (number === "") entries.splice(index, 1);
        else entries[index].number = number;
      } else if (number !== "") {
        if (entries.length >= 4096) {
          throw new Error("A document holds at most 4096 wire numbers.");
        }
        entries.push({ wireId: id, number });
      }
    });
  }

  saveSheet(value: Omit<SheetSpec, "id">, existingId?: StableId): StableId {
    assertPoint(value.origin);
    if (
      !value.name.trim() ||
      Array.from(value.name).length > 32 ||
      /\p{Cc}/u.test(value.name)
    ) {
      throw new Error(
        "Use a sheet name of 1 to 32 characters without control characters.",
      );
    }
    if (value.size !== "a4" && value.size !== "a3")
      throw new Error("Choose A4 or A3.");
    const sheets = this.schematic.documentation.sheets;
    if (existingId && !sheets.some((sheet) => sheet.id === existingId)) {
      throw new Error("The sheet no longer exists.");
    }
    if (!existingId && sheets.length >= 32)
      throw new Error("A project holds up to 32 sheets.");
    const sheet = {
      ...value,
      origin: { ...value.origin },
      id: existingId ?? this.nextId(),
    };
    if (
      sheets.some(
        (other) => other.id !== sheet.id && sheetsOverlap(sheet, other),
      )
    ) {
      throw new Error("Sheets cannot overlap.");
    }
    this.commit((draft) => {
      const index = draft.documentation.sheets.findIndex(
        (other) => other.id === sheet.id,
      );
      if (index === -1) draft.documentation.sheets.push(sheet);
      else draft.documentation.sheets[index] = sheet;
    });
    return sheet.id;
  }

  reorderSheet(id: StableId, offset: -1 | 1): void {
    this.commit((draft) => {
      const sheets = draft.documentation.sheets;
      const index = sheets.findIndex((sheet) => sheet.id === id);
      const target = index + offset;
      if (index < 0 || target < 0 || target >= sheets.length) return;
      [sheets[index], sheets[target]] = [sheets[target], sheets[index]];
    });
  }

  removeSheet(id: StableId): void {
    this.commit((draft) => {
      const index = draft.documentation.sheets.findIndex(
        (sheet) => sheet.id === id,
      );
      if (index >= 0) draft.documentation.sheets.splice(index, 1);
    });
  }

  setTitleBlockField(field: keyof TitleBlock, value: string): void {
    const limits: Record<keyof TitleBlock, number> = {
      title: 100,
      drawingNumber: 32,
      revision: 16,
      author: 64,
      company: 64,
      date: 32,
    };
    if (Array.from(value).length > limits[field] || /\p{Cc}/u.test(value)) {
      throw new Error(
        "The title block field is too long or contains a control character.",
      );
    }
    this.commit((draft) => {
      draft.documentation.titleBlock[field] = value;
    });
  }

  addWireNumbers(numbers: readonly WireNumberEntry[]): void {
    this.commit((draft) => {
      const entries = draft.documentation.wireNumbers;
      if (entries.length + numbers.length > 4096) {
        throw new Error("A document holds at most 4096 wire numbers.");
      }
      const unnumbered = new Set(draft.wires.map(({ id }) => id));
      for (const entry of entries) unnumbered.delete(entry.wireId);
      for (const entry of numbers) {
        if (
          !unnumbered.delete(entry.wireId) ||
          !validWireNumber(entry.number)
        ) {
          throw new Error("The wire numbering no longer matches the drawing.");
        }
        entries.push({ ...entry });
      }
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

  /**
   * Adds a vertical rail as one history entry and returns its ID. A rail is drawn input only:
   * adding one moves no wire, part or marker.
   */
  addRail(rail: NewRailSpec): StableId {
    const others = this.schematic.rails.map((item) => item.name);
    if (others.length >= MAX_RAILS) {
      throw new Error(`A schematic holds at most ${MAX_RAILS} rails.`);
    }
    const name = rail.name ?? nextRailName(others);
    assertRailName(name, others);
    assertRailExtent(rail.x, rail.top, rail.bottom);

    const id = this.nextId();
    const created = {
      id,
      name,
      start: { x: rail.x, y: rail.top },
      end: { x: rail.x, y: rail.bottom },
    };
    this.commit((draft) => {
      draft.rails.push(created);
    });
    return id;
  }

  /**
   * Pastes a copy as **one history entry** (docs/contracts/stage-16-editor-improvements.md
   * section 4): every object is added moved by `delta` with a **fresh stable ID**, so a pasted
   * object never keeps an ID of its source, and an undo removes exactly the pasted objects.
   * Parts are named by the automatic numbering when `designate` is set. A rail keeps its name
   * unless another rail has it, and then takes the next free one. Everything the store checks
   * for an added object (catalog and parameter domains, wire geometry, note text, limits) is
   * checked here, and the first refusal undoes the whole paste.
   */
  pasteClipboard(
    schematic: ClipboardSchematic,
    delta: GridPoint,
    catalog: ComponentCatalog,
    options: { readonly designate?: boolean } = {},
  ): void {
    assertPoint(delta);
    const move = (point: GridPoint): GridPoint => ({
      x: point.x + delta.x,
      y: point.y + delta.y,
    });

    this.transact(() => {
      for (const component of schematic.components) {
        // The source ID is dropped: the store issues a fresh one.
        const fresh = { ...component } as Partial<ComponentSpec>;
        delete fresh.id;
        this.addComponent(
          {
            ...fresh,
            position: move(component.position),
          } as NewComponentSpec,
          catalog,
          options,
        );
      }
      for (const wire of schematic.wires) {
        this.addWire({ points: wire.points.map(move) });
      }
      for (const junction of schematic.junctions) {
        this.addJunction(move(junction.position));
      }
      for (const marker of schematic.referenceMarkers) {
        this.addReferenceMarker(move(marker.position));
      }
      for (const rail of schematic.rails) {
        const taken = this.schematic.rails.some(
          (other) => other.name === rail.name,
        );
        const start = move(rail.start);
        const end = move(rail.end);
        this.addRail({
          ...(taken ? {} : { name: rail.name }),
          x: start.x,
          top: start.y,
          bottom: end.y,
        });
      }
      for (const note of schematic.notes) {
        this.addNote(move(note.position), note.text);
      }
    });
  }

  /**
   * Renames, moves or resizes a rail as one history entry; the rail keeps its ID. Wires, parts,
   * junctions and markers do not follow: attachment is geometric, not stored. An edit that
   * changes nothing records nothing.
   */
  editRail(id: StableId, patch: RailPatch): void {
    const current = this.schematic.rails.find((item) => item.id === id);
    if (!current) throw new Error(`Rail ${id} does not exist.`);

    const name = patch.name ?? current.name;
    const x = patch.x ?? current.start.x;
    const top = patch.top ?? current.start.y;
    const bottom = patch.bottom ?? current.end.y;
    assertRailName(
      name,
      this.schematic.rails
        .filter((item) => item.id !== id)
        .map((item) => item.name),
    );
    assertRailExtent(x, top, bottom);

    this.commit((draft) => {
      const rail = draft.rails.find((item) => item.id === id);
      if (!rail) throw new Error(`Rail ${id} does not exist.`);
      // Field by field: an unchanged value records no patch, so a no-op is no history entry.
      rail.name = name;
      rail.start.x = x;
      rail.start.y = top;
      rail.end.x = x;
      rail.end.y = bottom;
    });
  }

  /** Removes only the rail; wires and markers that touched it stay for editing. */
  removeRail(id: StableId): void {
    this.commit((draft) => {
      const index = draft.rails.findIndex((item) => item.id === id);
      if (index < 0) throw new Error(`Rail ${id} does not exist.`);
      draft.rails.splice(index, 1);
    });
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

  /**
   * Runs several edits as **one history entry**: one undo restores the document as it was, and
   * one redo repeats them all (docs/contracts/stage-16-editor-improvements.md section 3). If an
   * edit throws, the document is restored exactly, nothing is recorded, and the error is
   * rethrown. Nothing is published to listeners until the whole transaction has succeeded.
   * A transaction inside a transaction joins the outer one.
   */
  transact<T>(edits: () => T): T {
    if (this.batch) return edits();

    const start = this.schematic;
    const batch = {
      patches: [] as Patch[],
      inversePatches: [] as Patch[],
      topology: false,
    };
    this.batch = batch;
    let result: T;
    try {
      result = edits();
      if (batch.topology) {
        this.commit((draft) => this.followTerminals(draft, start));
      }
    } catch (error) {
      this.batch = null;
      this.schematic = start;
      throw error;
    }
    this.batch = null;

    if (batch.patches.length > 0) {
      this.undoStack.push({
        patches: batch.patches,
        inversePatches: batch.inversePatches,
      });
      this.redoStack.length = 0;
      this.publish();
    }
    return result;
  }

  private commit(recipe: (draft: Draft<Schematic>) => void): void {
    const [next, patches, inversePatches] = produceWithPatches(
      this.schematic,
      recipe,
    );
    if (patches.length === 0) return;
    this.schematic = next;
    if (this.batch) {
      // Patches apply in order, and their inverses in the reverse order.
      this.batch.patches.push(...patches);
      this.batch.inversePatches.unshift(...inversePatches);
      return;
    }
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
