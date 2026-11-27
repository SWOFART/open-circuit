import { useCallback, useEffect, useRef, useState } from "react";
import { Autosave } from "../store/autosave";
import type { SchematicDocumentStore } from "../store/document";
import type { RecoveryPort } from "../store/recoveryFiles";

export interface AutosaveController {
  /** "Autosave is not working: <reason>" while the last write failed; null otherwise. */
  readonly notice: string | null;
  /** Deletes the recovery file once any write in flight has finished. */
  readonly discard: () => Promise<void>;
}

/**
 * Autosave of the open document (stage 16 contract, section 9). It is on only while `enabled`
 * (a native side exists, no assignment is open, and the start-up offer is answered) and waits
 * while `blocked` (a save or an open is running).
 */
export function useAutosave(
  store: SchematicDocumentStore,
  port: RecoveryPort,
  enabled: boolean,
  blocked: boolean,
): AutosaveController {
  const [notice, setNotice] = useState<string | null>(null);
  const latest = useRef({ enabled: enabled && port.available, blocked });
  latest.current = { enabled: enabled && port.available, blocked };
  const autosave = useRef<Autosave | null>(null);

  useEffect(() => {
    const created = new Autosave(store, port, {
      enabled: () => latest.current.enabled,
      blocked: () => latest.current.blocked,
      onNotice: setNotice,
    });
    autosave.current = created;
    const stop = created.start();
    return () => {
      stop();
      autosave.current = null;
    };
  }, [store, port]);

  // A change of what is allowed lets the next write be planned (or cancels it).
  useEffect(() => {
    autosave.current?.poke();
  }, [enabled, blocked]);

  // No notice is kept for a write that autosave no longer makes.
  useEffect(() => {
    if (!enabled) setNotice(null);
  }, [enabled]);

  const discard = useCallback(
    () => autosave.current?.discard() ?? port.discard(),
    [port],
  );

  return { notice, discard };
}
