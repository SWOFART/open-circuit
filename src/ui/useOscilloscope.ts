import { useEffect, useMemo, useState, useSyncExternalStore } from "react";
import type { EngineAdapter } from "../store/engineAdapter";
import type { SimulationController } from "../store/simulation";
import {
  initialScopeState,
  OscilloscopeController,
  type ScopeState,
} from "../store/traces";

const noopSubscribe = (): (() => void) => () => {};

/**
 * Creates the oscilloscope controller for the lifetime of the simulation controller it
 * follows and exposes its state. It is disposed with that controller, which stops its
 * reads and ignores any answer still in flight.
 */
export function useOscilloscope(
  adapter: EngineAdapter | null,
  simulation: SimulationController | null,
): { scope: ScopeState; controller: OscilloscopeController | null } {
  const [controller, setController] = useState<OscilloscopeController | null>(
    null,
  );
  const idleState = useMemo(() => initialScopeState(), []);

  useEffect(() => {
    if (!adapter || !simulation) {
      return undefined;
    }

    const created = new OscilloscopeController(adapter, simulation);
    setController(created);
    created.start();

    return () => {
      created.dispose();
      setController(null);
    };
  }, [adapter, simulation]);

  const getSnapshot = controller ? controller.getState : () => idleState;
  const scope = useSyncExternalStore(
    controller ? controller.subscribe : noopSubscribe,
    getSnapshot,
    getSnapshot,
  );

  return { scope, controller };
}
