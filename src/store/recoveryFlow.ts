import type { SchematicDocumentStore } from "./document";
import type { ProjectFilePort } from "./projectFiles";
import type { RecoveryOffer, RecoveryPort } from "./recoveryFiles";

// What the start-up offer does (docs/contracts/stage-16-editor-improvements.md section 9).

/** What the offer dialog shows, or nothing when the application starts clean. */
export type RecoveryStartup =
  | { readonly kind: "none" }
  | ({ readonly kind: "offer" } & RecoveryOffer)
  | { readonly kind: "unreadable"; readonly reason: string };

/**
 * Looks for a recovery file before anything else. Without a native side, or when the look itself
 * fails, the application starts clean: a broken recovery folder must never keep it from starting.
 */
export async function loadRecoveryStartup(
  port: RecoveryPort,
): Promise<RecoveryStartup> {
  if (!port.available) {
    return { kind: "none" };
  }

  try {
    return await port.probe();
  } catch {
    return { kind: "none" };
  }
}

/**
 * Restore: opens the recovered document as an **unsaved** document with **no path**, so a plain
 * save asks for a place and can never overwrite the original project by surprise. The recovery
 * file stays until the first explicit save or an explicit discard.
 */
export async function restoreRecovery(
  offer: RecoveryOffer,
  document: SchematicDocumentStore,
  files: Pick<ProjectFilePort, "clearPath">,
): Promise<void> {
  await files.clearPath();
  document.replaceDocument(offer.schematic, { unsaved: true });
}
