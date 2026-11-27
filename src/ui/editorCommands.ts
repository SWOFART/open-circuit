import type { EditorCommand } from "../editor/commands";
import { DEFAULT_NOTE_TEXT } from "../editor/noteRules";
import type { ComponentCatalog } from "../generated/ComponentCatalog";
import type { ComponentTypeId } from "../generated/ComponentTypeId";
import type { ParameterId } from "../generated/ParameterId";
import type { Schematic } from "../generated/Schematic";
import type { StableId } from "../generated/StableId";
import type {
  NewComponentSpec,
  SchematicDocumentStore,
} from "../store/document";
import type { ParameterUpdate } from "../store/simulation";
import type { HistoryLabel } from "../editor/historyLabel";
import { message } from "./messages";
import {
  commandVerdict,
  parameterVerdict,
  type EditorRestrictions,
  type Verdict,
} from "../store/studentAssignment";

/**
 * A new component's parameters: every required catalog parameter at its
 * default, explicitly. Optional parameters, such as initial conditions, stay
 * absent: a default offered by the editor is never applied implicitly.
 */
export function defaultParameters(
  catalog: ComponentCatalog,
  typeId: ComponentTypeId,
): Record<string, number> {
  const entry = catalog.componentTypes.find((item) => item.typeId === typeId);
  if (!entry) {
    throw new Error(`Component type ${typeId} is not in the catalog.`);
  }

  return Object.fromEntries(
    entry.parameters
      .filter((parameter) => !parameter.optional)
      .map((parameter) => [parameter.id, parameter.defaultValue]),
  );
}

/**
 * The only live parameters of the Stage 10 devices (docs/contracts/stage-10-three-phase-equipment.md
 * section 7): the motor's load torque, the contactor's pickup and dropout thresholds and the
 * breaker's trip threshold. Every other input of these devices, and every input of the thermal
 * overload relay, the transformer and the three-phase loads, is compile-only: the Engine refuses
 * a live update of it, so it must start a new run.
 */
const LIVE_PARAMETERS_BY_TYPE: Readonly<
  Partial<Record<ComponentTypeId, readonly ParameterId[]>>
> = {
  induction_motor_three_phase: ["loadTorqueNewtonMeters"],
  contactor_three_pole: ["pickupCurrentAmps", "dropoutCurrentAmps"],
  breaker_three_pole: ["instantaneousTripAmps"],
  thermal_overload_three_pole: [],
  transformer_three_phase: [],
  three_phase_load_star: [],
  three_phase_load_delta: [],
  // Stage 13: the rated residual current and the trip time are live; the rest is compile-only.
  rcd_two_pole: ["ratedResidualCurrentAmps", "tripTimeAtRatedSeconds"],
  rcd_four_pole: ["ratedResidualCurrentAmps", "tripTimeAtRatedSeconds"],
  // The earth electrode is compile-only; the exposed part's inputs are live.
  earth_electrode: [],
  limit_switch: [],
  emergency_stop: [],
  selector_switch: [],
  key_switch: [],
  lamp: [],
  indicator: [],
  solenoid: [],
  valve: [],
  buzzer: [],
  frequency_converter: [],
  led: [],
  zener_diode: [],
  mosfet_n: [],
  operational_amplifier: [],
  relay_on_delay: [],
  relay_off_delay: [],
};

/**
 * The lightweight Engine update for a committed parameter edit, or undefined
 * when the edit must recompile the circuit. Initial conditions always start a
 * new run, as do inputs outside the live list for devices with explicit limits.
 */
export function liveParameterUpdate(
  catalog: ComponentCatalog,
  schematic: Schematic,
  componentId: StableId,
  parameterId: ParameterId,
  value: number,
): ParameterUpdate | undefined {
  const component = schematic.components.find(
    (item) => item.id === componentId,
  );
  const definition = catalog.componentTypes
    .find((entry) => entry.typeId === component?.typeId)
    ?.parameters.find((item) => item.id === parameterId);
  const live = component && LIVE_PARAMETERS_BY_TYPE[component.typeId];

  return definition &&
    !definition.initialCondition &&
    (live === undefined || live.includes(parameterId))
    ? { componentId, parameterId, value }
    : undefined;
}

/** What the history panel shows for the entry a command records. */
function historyLabelOf(
  command: EditorCommand,
  catalog: ComponentCatalog | null,
): HistoryLabel | null {
  switch (command.kind) {
    case "placeComponent": {
      const entry = catalog?.componentTypes.find(
        (item) => item.typeId === command.typeId,
      );
      return {
        key: "history.place",
        values: {
          name: entry ? message(entry.displayNameKey) : command.typeId,
        },
      };
    }
    case "transformComponent":
      return { key: "history.transform" };
    case "deleteComponent":
    case "deleteWire":
    case "deleteJunction":
    case "deleteReferenceMarker":
    case "deleteNote":
    case "deleteTerminal":
    case "deleteRail":
      return { key: "history.delete" };
    case "paste": {
      const { schematic } = command;
      return {
        key: "history.paste",
        values: {
          count:
            schematic.components.length +
            schematic.wires.length +
            schematic.junctions.length +
            schematic.referenceMarkers.length +
            schematic.rails.length +
            schematic.notes.length,
        },
      };
    }
    case "addWire":
      return { key: "history.drawWire" };
    case "editWire":
      return { key: "history.editWire" };
    case "addJunction":
      return { key: "history.addJunction" };
    case "addReferenceMarker":
      return { key: "history.addMarker" };
    case "addNote":
      return { key: "history.addNote" };
    case "addTerminal":
      return { key: "history.addTerminal" };
    case "moveJunction":
    case "moveReferenceMarker":
    case "moveNote":
      return { key: "history.moveObject" };
    case "addRail":
      return { key: "history.addRail" };
    case "editRail":
      return { key: "history.editRail" };
    case "batch":
      return command.label ?? null;
    default:
      return null;
  }
}

/**
 * Applies a canvas command to the document store as one history entry. The
 * store validates it against the catalog and throws, leaving the document
 * unchanged, when it is invalid. With `designate` a placed part is named in the same history
 * entry; it is left off while an assignment is open, which holds no documentation.
 */
export function applyEditorCommand(
  store: SchematicDocumentStore,
  command: EditorCommand,
  catalog: ComponentCatalog | null,
  designate = false,
): void {
  const label = historyLabelOf(command, catalog);
  if (label) {
    store.labelled(label, () =>
      applyLabelledCommand(store, command, catalog, designate),
    );
    return;
  }
  applyLabelledCommand(store, command, catalog, designate);
}

function applyLabelledCommand(
  store: SchematicDocumentStore,
  command: EditorCommand,
  catalog: ComponentCatalog | null,
  designate: boolean,
): void {
  switch (command.kind) {
    case "placeComponent": {
      if (!catalog) {
        throw new Error("The component catalog has not loaded yet.");
      }
      store.addComponent(
        {
          typeId: command.typeId,
          position: command.position,
          rotationDeg: command.rotationDeg,
          parameters: defaultParameters(catalog, command.typeId),
        } as NewComponentSpec,
        catalog,
        { designate },
      );
      return;
    }
    case "paste": {
      if (!catalog) {
        throw new Error("The component catalog has not loaded yet.");
      }
      store.pasteClipboard(command.schematic, command.delta, catalog, {
        designate,
      });
      return;
    }
    case "transformComponent":
      store.transformComponent(
        command.componentId,
        command.position,
        command.rotationDeg,
        command.wires,
        command.mirrored,
      );
      return;
    case "refuse":
      throw new Error(command.reason);
    case "deleteComponent":
      store.removeComponent(command.componentId);
      return;
    case "addWire":
      store.addWire({ points: [...command.points] });
      return;
    case "editWire":
      store.editWire(command.wireId, command.points);
      return;
    case "deleteWire":
      store.removeWire(command.wireId);
      return;
    case "addJunction":
      store.addJunction(command.position);
      return;
    case "moveJunction":
      store.moveJunction(command.id, command.position);
      return;
    case "deleteJunction":
      store.removeJunction(command.id);
      return;
    case "addReferenceMarker":
      store.addReferenceMarker(command.position);
      return;
    case "moveReferenceMarker":
      store.moveReferenceMarker(command.id, command.position);
      return;
    case "deleteReferenceMarker":
      store.removeReferenceMarker(command.id);
      return;
    case "addNote":
      store.addNote(command.position, DEFAULT_NOTE_TEXT);
      return;
    case "addTerminal": {
      const used = new Set(
        store
          .getState()
          .schematic.documentation.terminals.filter(
            (terminal) => terminal.strip === "X1",
          )
          .map((terminal) => terminal.label),
      );
      let number = 1;
      while (used.has(String(number))) number += 1;
      store.addTerminal(command.position, "X1", String(number));
      return;
    }
    case "deleteTerminal":
      store.removeTerminal(command.id);
      return;
    case "moveNote":
      store.moveNote(command.id, command.position);
      return;
    case "deleteNote":
      store.removeNote(command.id);
      return;
    case "addRail":
      store.addRail({
        ...(command.name === undefined ? {} : { name: command.name }),
        x: command.x,
        top: command.top,
        bottom: command.bottom,
      });
      return;
    case "editRail":
      store.editRail(command.railId, command.patch);
      return;
    case "deleteRail":
      store.removeRail(command.railId);
      return;
    case "batch":
      // One history entry for the whole group, and none if any part is refused.
      store.transact(() => {
        for (const part of command.commands) {
          applyEditorCommand(store, part, catalog, designate);
        }
      });
      return;
    case "undo":
      store.undo();
      return;
    case "redo":
      store.redo();
      return;
  }
}

/**
 * Applies a canvas command unless the open assignment's restrictions refuse it. A refused
 * command leaves the document untouched and says why; the document store's own rules are not
 * changed and still apply to a command that is allowed (it throws, as `applyEditorCommand`
 * does, when the store refuses it).
 */
export function applyRestrictedCommand(
  store: SchematicDocumentStore,
  command: EditorCommand,
  catalog: ComponentCatalog | null,
  restrictions: EditorRestrictions | null,
  designate = false,
): Verdict {
  const verdict = commandVerdict(command, restrictions);

  if (verdict.allowed) {
    applyEditorCommand(store, command, catalog, designate);
  }

  return verdict;
}

/** Sets a parameter unless the assignment locks the part. */
export function setRestrictedParameter(
  store: SchematicDocumentStore,
  componentId: StableId,
  parameterId: ParameterId,
  value: number,
  catalog: ComponentCatalog,
  restrictions: EditorRestrictions | null,
): Verdict {
  const verdict = parameterVerdict(componentId, restrictions);

  if (verdict.allowed) {
    store.labelled(
      {
        key: "history.setParameter",
        values: {
          part: store.getState().labels.get(componentId) ?? "part",
          parameter: parameterId,
        },
      },
      () => store.setParameter(componentId, parameterId, value, catalog),
    );
  }

  return verdict;
}

/** Clears an optional parameter unless the assignment locks the part. */
export function clearRestrictedParameter(
  store: SchematicDocumentStore,
  componentId: StableId,
  parameterId: ParameterId,
  catalog: ComponentCatalog,
  restrictions: EditorRestrictions | null,
): Verdict {
  const verdict = parameterVerdict(componentId, restrictions);

  if (verdict.allowed) {
    store.labelled(
      {
        key: "history.clearParameter",
        values: {
          part: store.getState().labels.get(componentId) ?? "part",
          parameter: parameterId,
        },
      },
      () => store.clearParameter(componentId, parameterId, catalog),
    );
  }

  return verdict;
}
