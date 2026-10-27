import type { ComponentSpec } from "../generated/ComponentSpec";
import type { DeviceState } from "../generated/DeviceState";
import type { ResultSnapshot } from "../generated/ResultSnapshot";
import type { Schematic } from "../generated/Schematic";
import type { StableId } from "../generated/StableId";
import type { ActiveRun, SimulationState } from "../store/simulation";
import { message } from "./messages";
import { formatSi, formatWithSymbol } from "./units";

export interface DevicePanelProps {
  readonly state: SimulationState;
  readonly schematic: Schematic;
  readonly labels: ReadonlyMap<StableId, string>;
}

/** The Stage 4 devices this panel reports on. */
const DEVICE_TYPES: ReadonlySet<ComponentSpec["typeId"]> = new Set([
  "relay_spdt",
  "fuse",
  "pmdc_motor",
  "voltmeter",
  "ammeter",
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
}

function describe(
  component: ComponentSpec,
  run: ActiveRun,
  snapshot: ResultSnapshot,
  labels: ReadonlyMap<StableId, string>,
): Device {
  const id = component.id;
  const name = labels.get(id) ?? component.typeId;
  const show = (key: string, unit: "volt" | "amp") => {
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
    default:
      throw new Error(`${component.typeId} is not a Stage 4 device.`);
  }
}

/**
 * Meter readings and the state of relays, fuses, and motors, exactly as the Engine
 * reported them for the run on screen: the discrete states come from the same frame as the
 * numbers, and nothing here is calculated. It is empty for a circuit without such devices.
 */
export function DevicePanel({ state, schematic, labels }: DevicePanelProps) {
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
