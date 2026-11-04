import { existsSync, readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { renderToStaticMarkup } from "react-dom/server";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { CompileRequest } from "../generated/CompileRequest";
import type { CompileResult } from "../generated/CompileResult";
import type { ComponentCatalog } from "../generated/ComponentCatalog";
import type { SetOperatorStateRequest } from "../generated/SetOperatorStateRequest";
import type { SetOperatorStateResult } from "../generated/SetOperatorStateResult";
import { symbols } from "../symbols";
import { DevicePanel } from "../ui/DevicePanel";
import { operatorPosition } from "../ui/operatorPosition";
import { SchematicDocumentStore } from "./document";
import { TauriEngineAdapter } from "./engineAdapter";
import { SimulationController } from "./simulation";

// Only the Tauri transport is replayed. Every reply and OCF2 byte comes from the real
// Engine in stage6_acceptance.rs; every outgoing request must match the recorded request.
interface Circuit {
  request: CompileRequest;
  compileReply: CompileResult & { status: "success" };
  frames: { step: number; hex: string }[];
  updates: {
    afterStep: number;
    request: SetOperatorStateRequest;
    reply: SetOperatorStateResult & { status: "success" };
  }[];
  resetRequest: CompileRequest;
  resetReply: CompileResult & { status: "success" };
  projectText: string;
}
interface Session {
  catalog: ComponentCatalog;
  circuits: Record<string, Circuit>;
}
const sessionPath = fileURLToPath(
  new URL("../../target/tmp/stage6-session.json", import.meta.url),
);
const recorded = existsSync(sessionPath);
if (!recorded && process.env.CI)
  throw new Error(
    "Run cargo test before Vitest to record the Stage 6 Engine session.",
  );
const session = (
  recorded ? JSON.parse(readFileSync(sessionPath, "utf8")) : {}
) as Session;

const transport = vi.hoisted(() => ({
  answer: (() => undefined) as (
    command: string,
    args?: Record<string, unknown>,
  ) => unknown,
  channel: null as { onmessage: (bytes: ArrayBuffer) => void } | null,
  class: class {
    onmessage: (bytes: ArrayBuffer) => void = () => {};
  },
}));
vi.mock("@tauri-apps/api/core", () => ({
  Channel: transport.class,
  isTauri: () => true,
  invoke: async (command: string, args?: Record<string, unknown>) => {
    if (command === "start_simulation")
      transport.channel = args?.onFrame as typeof transport.channel;
    return transport.answer(command, args);
  },
}));

function deliver(hex: string) {
  transport.channel!.onmessage(Uint8Array.from(Buffer.from(hex, "hex")).buffer);
}

async function boot(circuit: Circuit) {
  let compiles = 0;
  let updates = 0;
  transport.answer = (command, args) => {
    switch (command) {
      case "get_component_catalog":
        return session.catalog;
      case "compile_circuit": {
        const reset = compiles++ > 0;
        expect(args?.request).toEqual(
          reset ? circuit.resetRequest : circuit.request,
        );
        return reset ? circuit.resetReply : circuit.compileReply;
      }
      case "set_operator_state": {
        const update = circuit.updates[updates++]!;
        expect(args?.request).toEqual(update.request);
        return update.reply;
      }
      case "start_simulation":
      case "stop_simulation":
      case "ack_frame":
        return undefined;
      default:
        throw new Error(`Unexpected Tauri command ${command}`);
    }
  };
  const store = new SchematicDocumentStore(circuit.request.schematic);
  let revision = 0;
  const controller = new SimulationController(
    new TauriEngineAdapter(),
    symbols,
    () => `rev-${++revision}`,
  );
  await controller.start(store.getState().schematic);
  return { store, controller, counts: () => ({ compiles, updates }) };
}

function panel(
  controller: SimulationController,
  store: SchematicDocumentStore,
) {
  return renderToStaticMarkup(
    <DevicePanel
      state={controller.getState()}
      schematic={store.getState().schematic}
      labels={store.getState().labels}
      onOperate={() => {}}
    />,
  );
}

beforeEach(() => {
  vi.useFakeTimers();
  transport.channel = null;
});
afterEach(() => {
  vi.useRealTimers();
});

describe.skipIf(!recorded)(
  "Stage 6 editor circuits through the real Engine recording",
  () => {
    it("does not restore an old operator reply while a drawing edit waits for it", async () => {
      const circuit = session.circuits.switch!;
      const { store, controller } = await boot(circuit);
      try {
        const answer = transport.answer;
        let finish = () => {};
        transport.answer = (command, args) => {
          if (command === "set_operator_state") {
            const reply = answer(command, args);
            return new Promise((resolve) => {
              finish = () => resolve(reply);
            });
          }
          if (command === "compile_circuit") {
            const request = args?.request as CompileRequest;
            expect(request).toEqual({
              ...circuit.resetRequest,
              documentRevision: "rev-3",
            });
            return {
              ...circuit.resetReply,
              initialSnapshot: {
                ...circuit.resetReply.initialSnapshot,
                metadata: {
                  ...circuit.resetReply.initialSnapshot.metadata,
                  documentRevision: request.documentRevision,
                },
              },
            };
          }
          return answer(command, args);
        };
        const operating = controller.setOperatorState(
          circuit.updates[0]!.request.componentId,
          "closed",
        );
        const compiling = controller.syncDocument(
          circuit.resetRequest.schematic,
        );
        expect(controller.getState().status).toBe("loading");
        expect(controller.getState().snapshot).toBeNull();
        expect(panel(controller, store)).not.toContain("aria-checked=");
        const visible: string[] = [];
        const unsubscribe = controller.subscribe(() => {
          if (controller.getState().snapshot)
            visible.push(controller.getState().snapshot!.metadata.runId);
        });
        finish();
        await operating;
        await compiling;
        unsubscribe();
        expect(visible.length).toBeGreaterThan(0);
        expect(
          visible.every((runId) => runId === circuit.resetReply.runId),
        ).toBe(true);
      } finally {
        controller.dispose();
      }
    });

    it("withdraws the old run while the changed document is still compiling", async () => {
      const circuit = session.circuits.switch!;
      const { store, controller } = await boot(circuit);
      try {
        const answer = transport.answer;
        let finish: (reply: CompileResult) => void = () => {};
        transport.answer = (command, args) =>
          command === "compile_circuit"
            ? new Promise<CompileResult>((resolve) => {
                const reply = circuit.resetReply;
                finish = () =>
                  resolve({
                    ...reply,
                    initialSnapshot: {
                      ...reply.initialSnapshot,
                      metadata: {
                        ...reply.initialSnapshot.metadata,
                        documentRevision: (args?.request as CompileRequest)
                          .documentRevision,
                      },
                    },
                  });
              })
            : answer(command, args);
        const oldChannel = transport.channel!;
        const pending = controller.syncDocument(circuit.resetRequest.schematic);
        expect(controller.getState().status).toBe("loading");
        expect(controller.getState().run).toBeNull();
        expect(controller.getState().snapshot).toBeNull();
        expect(panel(controller, store)).not.toContain("aria-checked=");
        oldChannel.onmessage(
          Uint8Array.from(Buffer.from(circuit.frames[0]!.hex, "hex")).buffer,
        );
        expect(controller.getState().snapshot).toBeNull();
        finish(circuit.resetReply);
        await pending;
        expect(controller.getState().run?.runId).toBe(circuit.resetReply.runId);
      } finally {
        controller.dispose();
      }
    });

    for (const name of ["switch", "button", "latching", "unlatched"]) {
      it(`shows ${name} operator changes, rejects stale frames, and resets after a drawing edit`, async () => {
        const circuit = session.circuits[name]!;
        const { store, controller, counts } = await boot(circuit);
        try {
          const before = store.getState();
          for (const frame of circuit.frames) {
            deliver(frame.hex);
            for (const update of circuit.updates.filter(
              (u) => u.afterStep === frame.step,
            )) {
              const state = controller
                .getState()
                .snapshot!.states.find(
                  (s) => s.componentId === update.request.componentId,
                );
              const action =
                update.request.position === "closed" ||
                update.request.position === "open"
                  ? "toggle"
                  : update.request.position === "pressed"
                    ? "press"
                    : "release";
              const position = operatorPosition(action, state);
              expect(position).toBe(update.request.position);
              await controller.setOperatorState(
                update.request.componentId,
                position!,
              );
              const current = controller.getState();
              expect(current.snapshot?.states).toEqual(
                update.reply.snapshot.states,
              );
              expect(current.snapshot?.values).toEqual(
                update.reply.snapshot.values,
              );
              expect(current.run?.runId).toBe(circuit.compileReply.runId);
              const markup = panel(controller, store);
              expect(markup).toContain(
                update.request.position === "pressed"
                  ? 'aria-pressed="true"'
                  : update.request.position === "released"
                    ? 'aria-pressed="false"'
                    : update.request.position === "closed"
                      ? 'aria-checked="true"'
                      : 'aria-checked="false"',
              );
              // A late frame from immediately before the update cannot restore its old state/value.
              deliver(frame.hex);
              expect(controller.getState().snapshot).toBe(current.snapshot);
              expect(controller.getState().discardedFrames).toBe(
                current.discardedFrames + 1,
              );
            }
          }
          expect(counts()).toEqual({
            compiles: 1,
            updates: circuit.updates.length,
          });
          expect(store.getState()).toBe(before);
          expect(store.getState().isDirty).toBe(false);
          expect(store.getState().canUndo).toBe(false);
          expect(JSON.parse(circuit.projectText)).toEqual(
            store.getState().schematic,
          );
          // The drawing is translated by one grid unit, preserving topology and forcing a compile.
          await controller.syncDocument(circuit.resetRequest.schematic);
          expect(counts().compiles).toBe(2);
          const reset = controller.getState();
          expect(reset.run?.runId).toBe(circuit.resetReply.runId);
          expect(reset.snapshot?.states).toEqual(
            circuit.compileReply.initialSnapshot.states,
          );
          expect(reset.snapshot?.values).toEqual(
            circuit.compileReply.initialSnapshot.values,
          );
          deliver(circuit.frames.at(-1)!.hex);
          expect(controller.getState().snapshot).toBe(reset.snapshot);
          expect(controller.getState().discardedFrames).toBe(
            reset.discardedFrames + 1,
          );
        } finally {
          controller.dispose();
        }
      });
    }
  },
);
