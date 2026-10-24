import { useEffect, useMemo, useState, useSyncExternalStore } from "react";
import type { Schematic } from "../generated/Schematic";
import { createEngineAdapter } from "../store/engineAdapter";
import {
  initialSimulationState,
  SimulationController,
  type SimulationState,
} from "../store/simulation";

const noopSubscribe = (): (() => void) => () => {};

/**
 * Creates the controller for the lifetime of the component and exposes its
 * state. The controller is disposed on unmount, which also drops its
 * subscriptions and ignores any response still in flight.
 */
export function useSimulation(schematic: Schematic): {
  state: SimulationState;
  controller: SimulationController | null;
} {
  const [controller, setController] = useState<SimulationController | null>(
    null,
  );
  const idleState = useMemo(
    () => initialSimulationState(schematic),
    [schematic],
  );

  useEffect(() => {
    let cancelled = false;
    let created: SimulationController | null = null;

    void createEngineAdapter().then((adapter) => {
      if (cancelled) {
        return;
      }

      created = new SimulationController(adapter, schematic);
      setController(created);
      void created.start();
    });

    return () => {
      cancelled = true;
      created?.dispose();
      setController(null);
    };
  }, [schematic]);

  const getSnapshot = controller ? controller.getState : () => idleState;
  const state = useSyncExternalStore(
    controller ? controller.subscribe : noopSubscribe,
    getSnapshot,
    getSnapshot,
  );

  return { state, controller };
}
