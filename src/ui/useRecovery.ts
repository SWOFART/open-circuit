import { useCallback, useEffect, useState } from "react";
import type { RecoveryPort } from "../store/recoveryFiles";
import {
  loadRecoveryStartup,
  type RecoveryStartup,
} from "../store/recoveryFlow";

export interface RecoveryController {
  /** What the offer dialog shows; null while there is nothing to offer (or nothing yet). */
  readonly pending: Exclude<RecoveryStartup, { kind: "none" }> | null;
  /** True once the start-up look is over and the offer, if any, has been answered. */
  readonly resolved: boolean;
  readonly busy: boolean;
  readonly error: string | null;
  readonly restore: () => void;
  readonly discard: () => void;
}

/**
 * The offer at start: looks once for a recovery file, and answers it with Restore (through
 * `onRestore`, which opens the document) or Discard (which deletes the file). The offer waits;
 * nothing else resolves it.
 */
export function useRecovery(
  port: RecoveryPort,
  onRestore: (
    offer: Extract<RecoveryStartup, { kind: "offer" }>,
  ) => Promise<void>,
): RecoveryController {
  const [startup, setStartup] = useState<RecoveryStartup | "checking">(
    "checking",
  );
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let current = true;
    void loadRecoveryStartup(port).then((found) => {
      if (current) setStartup(found);
    });
    return () => {
      current = false;
    };
  }, [port]);

  const answer = useCallback(
    async (work: () => Promise<void>): Promise<void> => {
      setBusy(true);
      setError(null);
      try {
        await work();
        setStartup({ kind: "none" });
      } catch (reason) {
        setError(reason instanceof Error ? reason.message : String(reason));
      } finally {
        setBusy(false);
      }
    },
    [],
  );

  const restore = useCallback(() => {
    if (typeof startup === "object" && startup.kind === "offer") {
      void answer(() => onRestore(startup));
    }
  }, [answer, onRestore, startup]);

  const discard = useCallback(() => {
    void answer(() => port.discard());
  }, [answer, port]);

  return {
    pending:
      typeof startup === "object" && startup.kind !== "none" ? startup : null,
    resolved: typeof startup === "object" && startup.kind === "none",
    busy,
    error,
    restore,
    discard,
  };
}
