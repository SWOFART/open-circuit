import type { OperateAction } from "../editor/interaction";
import type { ComponentSpec } from "../generated/ComponentSpec";
import type { DeviceState } from "../generated/DeviceState";
import type { ResultSnapshot } from "../generated/ResultSnapshot";
import type { Schematic } from "../generated/Schematic";
import type { StableId } from "../generated/StableId";
import type { Unit } from "../generated/Unit";
import { measurementValidityKey } from "../store/measurementValidity";
import type { ActiveRun, SimulationState } from "../store/simulation";
import { message } from "./messages";
import { slotDescription } from "./slotLabels";
import { OperatorControl } from "./OperatorControl";
import { formatSi, formatWithSymbol } from "./units";

export interface DevicePanelProps {
  readonly state: SimulationState;
  readonly schematic: Schematic;
  readonly labels: ReadonlyMap<StableId, string>;
  /** Sends a switch or push button position to the Engine. */
  readonly onOperate?: (componentId: StableId, action: OperateAction) => void;
}

/** The Stage 4 and Stage 6 devices this panel reports on. */
const DEVICE_TYPES: ReadonlySet<ComponentSpec["typeId"]> = new Set([
  "relay_spdt",
  "fuse",
  "contactor_three_pole",
  "breaker_three_pole",
  "thermal_overload_three_pole",
  "induction_motor_three_phase",
  "transformer_three_phase",
  "three_phase_load_star",
  "three_phase_load_delta",
  "pmdc_motor",
  "voltmeter",
  "ammeter",
  "ac_voltmeter",
  "ac_ammeter",
  "wattmeter",
  "switch_spst",
  "push_button_no",
  "push_button_nc",
  "contactor",
  "diode",
  "bjt_npn",
  "bjt_pnp",
]);

export function hasDevices(schematic: Schematic): boolean {
  return schematic.components.some((component) =>
    DEVICE_TYPES.has(component.typeId),
  );
}

/**
 * One value of the run, found by the stable key the Engine gave its slot
 * (docs/contracts/stage-4-components.md section 5) and read through the layout's
 * offset, like every other result.
 */
function reading(
  run: ActiveRun,
  snapshot: ResultSnapshot,
  key: string,
): number | undefined {
  const slot = run.layout.slots.find((candidate) => candidate.key === key);

  return slot ? snapshot.values[slot.offset] : undefined;
}

function stateOf(
  snapshot: ResultSnapshot,
  id: StableId,
  kind: DeviceState["kind"],
): DeviceState | undefined {
  return snapshot.states.find(
    (candidate) => candidate.componentId === id && candidate.kind === kind,
  );
}

function transition(state: DeviceState | undefined): string | null {
  return state?.lastTransitionTimeSeconds === undefined
    ? null
    : `since ${formatWithSymbol(state.lastTransitionTimeSeconds, "s")}`;
}

interface Row {
  readonly name: string;
  readonly value: string;
}

interface Device {
  readonly id: StableId;
  readonly title: string;
  /** A state the Engine reports, shown as a badge. */
  readonly badge?: { readonly text: string; readonly tone: string };
  readonly rows: readonly Row[];
  readonly resettable?: boolean;
  /** A switch or push button the user can operate, with the position the Engine reported. */
  readonly control?: {
    readonly kind: "switch" | "pushButton";
    readonly name: string;
    readonly actuated: boolean;
    /** What the control is called in its accessible name; a switch by default. */
    readonly noun?: string;
  };
}

function yesNo(value: boolean): string {
  return value ? "Yes" : "No";
}

/** The three pole currents of a switching device, as the Engine reported them. */
function poleRows(
  id: StableId,
  show: (key: string, unit: Unit) => string,
): readonly Row[] {
  return [1, 2, 3].map((pole) => ({
    name: `Pole L${pole} current`,
    value: show(`${id}/pole${pole}/current`, "amp"),
  }));
}

function describe(
  component: ComponentSpec,
  run: ActiveRun,
  snapshot: ResultSnapshot,
  labels: ReadonlyMap<StableId, string>,
): Device {
  const id = component.id;
  const name = labels.get(id) ?? component.typeId;
  const show = (key: string, unit: Unit) => {
    const value = reading(run, snapshot, key);

    return value === undefined ? "—" : formatSi(value, unit);
  };

  switch (component.typeId) {
    case "relay_spdt": {
      const state = stateOf(snapshot, id, "relay");
      const since = transition(state);

      return {
        id,
        title: `${name} ${message("component.relaySpdt.name")}`,
        badge:
          state?.kind === "relay"
            ? {
                text: state.state === "picked" ? "Picked" : "Released",
                tone: state.state === "picked" ? "on" : "off",
              }
            : undefined,
        rows: [
          ...(since === null ? [] : [{ name: "Last change", value: since }]),
          { name: "Coil current", value: show(`${id}/coil/current`, "amp") },
        ],
      };
    }
    case "contactor_three_pole": {
      const state = stateOf(snapshot, id, "contactor");
      const since = transition(state);
      const picked = state?.kind === "contactor" && state.state === "picked";

      return {
        id,
        title: `${name} ${message("component.contactorThreePole.name")}`,
        badge:
          state?.kind === "contactor"
            ? {
                text: picked ? "Picked" : "Released",
                tone: picked ? "on" : "off",
              }
            : undefined,
        rows: [
          ...(since === null ? [] : [{ name: "Last change", value: since }]),
          { name: "Coil current", value: show(`${id}/coil/current`, "amp") },
          {
            name: "Coil detector (mean square)",
            value: show(`${id}/coil/current_squared`, "ampere_squared"),
          },
          ...poleRows(id, show),
          {
            name: "Normally open contact current",
            value: show(`${id}/no/current`, "amp"),
          },
          {
            name: "Normally closed contact current",
            value: show(`${id}/nc/current`, "amp"),
          },
        ],
      };
    }
    case "breaker_three_pole": {
      const state = stateOf(snapshot, id, "switch");
      const since = transition(state);
      const closed = state?.kind === "switch" && state.position === "closed";
      const tripped = reading(run, snapshot, `${id}/tripped`) === 1;

      return {
        id,
        title: `${name} ${message("component.breakerThreePole.name")}`,
        badge:
          state?.kind === "switch"
            ? tripped
              ? { text: "Tripped", tone: "fault" }
              : {
                  text: closed ? "Closed" : "Open",
                  tone: closed ? "on" : "off",
                }
            : undefined,
        control: { kind: "switch", name, actuated: closed, noun: "breaker" },
        rows: [
          ...(tripped ? [{ name: "To reset", value: "Open the breaker" }] : []),
          ...(state?.kind === "switch"
            ? [{ name: "Conducting", value: yesNo(state.conducting) }]
            : []),
          ...(since === null ? [] : [{ name: "Last change", value: since }]),
          ...poleRows(id, show),
        ],
      };
    }
    case "thermal_overload_three_pole": {
      const state = stateOf(snapshot, id, "fuse");
      const since = transition(state);
      const tripped = state?.kind === "fuse" && state.state === "tripped";
      const dose = reading(run, snapshot, `${id}/thermal_dose`);
      return {
        id,
        title: `${name} ${message("component.thermalOverloadThreePole.name")}`,
        badge: {
          text: tripped ? "Tripped" : "Intact",
          tone: tripped ? "fault" : "on",
        },
        resettable: tripped,
        rows: [
          ...(since === null ? [] : [{ name: "Last change", value: since }]),
          {
            name: "Thermal dose",
            value: dose === undefined ? "—" : formatSi(dose, "dimensionless"),
          },
          ...poleRows(id, show),
        ],
      };
    }
    case "fuse": {
      const state = stateOf(snapshot, id, "fuse");
      const since = transition(state);
      const accumulated = reading(run, snapshot, `${id}/accumulated_i2t`);
      const limit = component.parameters.tripI2tAmpereSquaredSeconds;

      return {
        id,
        title: `${name} ${message("component.fuse.name")}`,
        badge:
          state?.kind === "fuse"
            ? {
                text: state.state === "tripped" ? "Tripped" : "Intact",
                tone: state.state === "tripped" ? "fault" : "on",
              }
            : undefined,
        rows: [
          ...(since === null ? [] : [{ name: "Tripped", value: since }]),
          {
            name: "Accumulated I²t",
            value:
              accumulated === undefined
                ? "—"
                : `${formatSi(accumulated, "ampere_squared_second")} of ${formatSi(limit, "ampere_squared_second")}`,
          },
          { name: "Current", value: show(`${id}:current`, "amp") },
        ],
      };
    }
    case "switch_spst": {
      const state = stateOf(snapshot, id, "switch");
      const since = transition(state);
      const closed = state?.kind === "switch" && state.position === "closed";

      return {
        id,
        title: `${name} ${message("component.switchSpst.name")}`,
        badge:
          state?.kind === "switch"
            ? { text: closed ? "Closed" : "Open", tone: closed ? "on" : "off" }
            : undefined,
        control: { kind: "switch", name, actuated: closed },
        rows: [
          ...(state?.kind === "switch"
            ? [{ name: "Conducting", value: yesNo(state.conducting) }]
            : []),
          ...(since === null ? [] : [{ name: "Last change", value: since }]),
          { name: "Current", value: show(`${id}:current`, "amp") },
          { name: "Voltage", value: show(`${id}:voltage`, "volt") },
        ],
      };
    }
    case "push_button_no":
    case "push_button_nc": {
      const state = stateOf(snapshot, id, "pushButton");
      const since = transition(state);
      const pressed =
        state?.kind === "pushButton" && state.position === "pressed";

      return {
        id,
        title: `${name} ${message(
          component.typeId === "push_button_no"
            ? "component.pushButtonNo.name"
            : "component.pushButtonNc.name",
        )}`,
        badge:
          state?.kind === "pushButton"
            ? {
                text: pressed ? "Pressed" : "Released",
                tone: pressed ? "on" : "off",
              }
            : undefined,
        control: { kind: "pushButton", name, actuated: pressed },
        rows: [
          ...(state?.kind === "pushButton"
            ? [{ name: "Conducting", value: yesNo(state.conducting) }]
            : []),
          ...(since === null ? [] : [{ name: "Last change", value: since }]),
          { name: "Current", value: show(`${id}:current`, "amp") },
          { name: "Voltage", value: show(`${id}:voltage`, "volt") },
        ],
      };
    }
    case "contactor": {
      const state = stateOf(snapshot, id, "contactor");
      const since = transition(state);
      const picked = state?.kind === "contactor" && state.state === "picked";

      return {
        id,
        title: `${name} ${message("component.contactor.name")}`,
        badge:
          state?.kind === "contactor"
            ? {
                text: picked ? "Picked" : "Released",
                tone: picked ? "on" : "off",
              }
            : undefined,
        rows: [
          ...(since === null ? [] : [{ name: "Last change", value: since }]),
          { name: "Coil current", value: show(`${id}/coil/current`, "amp") },
          {
            name: "Normally open contact current",
            value: show(`${id}/no/current`, "amp"),
          },
          {
            name: "Normally closed contact current",
            value: show(`${id}/nc/current`, "amp"),
          },
        ],
      };
    }
    case "diode":
      // A diode has no state of its own: the Engine reports only the voltage across it and the
      // current through it, positive entering the anode.
      return {
        id,
        title: `${name} ${message("component.diode.name")}`,
        rows: [
          {
            name: "Voltage (anode − cathode)",
            value: show(`${id}:voltage`, "volt"),
          },
          {
            name: "Current (into the anode)",
            value: show(`${id}:current`, "amp"),
          },
        ],
      };
    case "bjt_npn":
    case "bjt_pnp": {
      // The five numbers are the Engine's, read by their keys; the interface derives none of
      // them, and shows no operating region (the contract defines none).
      return {
        id,
        title: `${name} ${message(
          component.typeId === "bjt_npn"
            ? "component.bjtNpn.name"
            : "component.bjtPnp.name",
        )}`,
        rows: [
          {
            name: "Base-emitter voltage",
            value: show(`${id}/base_emitter/voltage`, "volt"),
          },
          {
            name: "Collector-emitter voltage",
            value: show(`${id}/collector_emitter/voltage`, "volt"),
          },
          {
            name: "Collector current (into the collector)",
            value: show(`${id}/collector/current`, "amp"),
          },
          {
            name: "Base current (into the base)",
            value: show(`${id}/base/current`, "amp"),
          },
          {
            name: "Emitter current (into the emitter)",
            value: show(`${id}/emitter/current`, "amp"),
          },
        ],
      };
    }
    case "induction_motor_three_phase": {
      const speed = reading(run, snapshot, `${id}/angular_velocity`);

      return {
        id,
        title: `${name} ${message("component.inductionMotorThreePhase.name")}`,
        rows: [
          {
            name: "Angular velocity",
            value:
              speed === undefined ? "—" : formatSi(speed, "radian_per_second"),
          },
          {
            name: "Electromagnetic torque",
            value: show(`${id}/electromagnetic_torque`, "newton_meter"),
          },
          ...["u", "v", "w"].map((winding) => ({
            name: `Winding ${winding.toUpperCase()} current`,
            value: show(`${id}/${winding}/current`, "amp"),
          })),
        ],
      };
    }
    case "transformer_three_phase":
      return {
        id,
        title: `${name} ${message("component.transformerThreePhase.name")}`,
        rows: [1, 2, 3].flatMap((bank) => [
          {
            name: `Primary ${bank} current`,
            value: show(`${id}/primary${bank}/current`, "amp"),
          },
          {
            name: `Secondary ${bank} current`,
            value: show(`${id}/secondary${bank}/current`, "amp"),
          },
          {
            name: `Secondary ${bank} voltage`,
            value: show(`${id}/secondary${bank}/voltage`, "volt"),
          },
        ]),
      };
    case "three_phase_load_star":
      return {
        id,
        title: `${name} ${message("component.threePhaseLoadStar.name")}`,
        rows: [
          ...[1, 2, 3].map((line) => ({
            name: `Branch L${line}–N current`,
            value: show(`${id}/l${line}/current`, "amp"),
          })),
          { name: "Neutral current", value: show(`${id}/n/current`, "amp") },
        ],
      };
    case "three_phase_load_delta":
      return {
        id,
        title: `${name} ${message("component.threePhaseLoadDelta.name")}`,
        rows: [
          ...[
            ["l12", "L1–L2"],
            ["l23", "L2–L3"],
            ["l31", "L3–L1"],
          ].map(([group, label]) => ({
            name: `Branch ${label} current`,
            value: show(`${id}/${group}/current`, "amp"),
          })),
          ...[1, 2, 3].map((line) => ({
            name: `Line L${line} current`,
            value: show(`${id}/l${line}/current`, "amp"),
          })),
        ],
      };
    case "pmdc_motor": {
      const speed = reading(run, snapshot, `${id}/angular_velocity`);

      return {
        id,
        title: `${name} ${message("component.pmdcMotor.name")}`,
        rows: [
          {
            name: "Angular velocity",
            value:
              speed === undefined ? "—" : formatSi(speed, "radian_per_second"),
          },
          { name: "Armature current", value: show(`${id}:current`, "amp") },
          { name: "Terminal voltage", value: show(`${id}:voltage`, "volt") },
        ],
      };
    }
    case "voltmeter":
      return {
        id,
        title: `${name} ${message("component.voltmeter.name")}`,
        rows: [
          { name: "Reading", value: show(`${id}:voltage`, "volt") },
          { name: "Input current", value: show(`${id}:current`, "amp") },
        ],
      };
    case "ammeter":
      return {
        id,
        title: `${name} ${message("component.ammeter.name")}`,
        rows: [
          { name: "Reading", value: show(`${id}:current`, "amp") },
          { name: "Burden voltage", value: show(`${id}:voltage`, "volt") },
        ],
      };
    case "ac_voltmeter":
    case "ac_ammeter":
    case "wattmeter": {
      const title = {
        ac_voltmeter: "component.acVoltmeter.name",
        ac_ammeter: "component.acAmmeter.name",
        wattmeter: "component.wattmeter.name",
      }[component.typeId];
      const slots = run.layout.slots.filter(
        (slot) => slot.componentId === id && slot.quantity !== "valid",
      );
      return {
        id,
        title: `${name} ${message(title)}`,
        rows: slots.map((slot) => {
          const flag = measurementValidityKey(slot);
          const available =
            flag === undefined || reading(run, snapshot, flag) === 1;
          const value = snapshot.values[slot.offset];
          return {
            name: slotDescription(slot),
            value:
              available && value !== undefined
                ? formatSi(value, slot.unit)
                : "Not yet valid",
          };
        }),
      };
    }
    default:
      throw new Error(`${component.typeId} is not a device of this panel.`);
  }
}

/**
 * Meter readings and the state of relays, fuses, and motors, exactly as the Engine
 * reported them for the run on screen: the discrete states come from the same frame as the
 * numbers, and nothing here is calculated. It is empty for a circuit without such devices.
 */
export function DevicePanel({
  state,
  schematic,
  labels,
  onOperate,
}: DevicePanelProps) {
  const { run, snapshot } = state;

  if (!hasDevices(schematic)) {
    return null;
  }

  const devices =
    state.status === "ready" && run && snapshot
      ? schematic.components
          .filter((component) => DEVICE_TYPES.has(component.typeId))
          .map((component) => describe(component, run, snapshot, labels))
      : [];

  return (
    <section className="side-panel" aria-labelledby="devices-heading">
      <h2 id="devices-heading">Devices</h2>
      {devices.length === 0 ? (
        <p className="panel-note" role="status">
          {state.status === "failed"
            ? "No active run: the engine could not calculate this circuit."
            : "Waiting for the engine…"}
        </p>
      ) : (
        <ul className="devices">
          {devices.map((device) => (
            <li key={device.id} className="device">
              <p className="device-title">
                <span>{device.title}</span>
                {device.badge && (
                  <span
                    className={`device-badge device-badge-${device.badge.tone}`}
                  >
                    {device.badge.text}
                  </span>
                )}
              </p>
              {device.control && (
                <OperatorControl
                  componentId={device.id}
                  name={device.control.name}
                  kind={device.control.kind}
                  noun={device.control.noun}
                  actuated={device.control.actuated}
                  disabled={
                    state.status !== "ready" || state.run === null || !onOperate
                  }
                  onOperate={(componentId, action) =>
                    onOperate?.(componentId, action)
                  }
                />
              )}
              {device.resettable && onOperate && (
                <button
                  type="button"
                  onClick={() => onOperate(device.id, "reset")}
                >
                  Reset overload
                </button>
              )}
              <dl className="readings">
                {device.rows.map((row) => (
                  <div key={row.name} className="reading">
                    <dt>{row.name}</dt>
                    <dd>{row.value}</dd>
                  </div>
                ))}
              </dl>
            </li>
          ))}
        </ul>
      )}
      {state.status === "ready" && state.updating && devices.length > 0 && (
        <p className="panel-note" role="status">
          Updating…
        </p>
      )}
    </section>
  );
}
