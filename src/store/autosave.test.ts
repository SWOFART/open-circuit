import { describe, expect, it } from "vitest";
import type { Schematic } from "../generated/Schematic";
import {
  Autosave,
  AUTOSAVE_IDLE_MS,
  AUTOSAVE_INTERVAL_MS,
  type AutosaveClock,
} from "./autosave";
import { SchematicDocumentStore } from "./document";
import { dividerSchematic } from "./fixtures";

// Autosave (docs/contracts/stage-16-editor-improvements.md section 9): 2 seconds without an
// edit, then at most every 30 seconds, never while a save or an open runs, and a failed write is
// a notice and a retry, not an interruption.

const R1 = "00000000-0000-4000-8000-000000000002";

/** A clock whose time only moves when a test says so. */
class FakeClock implements AutosaveClock {
  time = 1_000_000;
  private nextId = 1;
  private readonly timers = new Map<
    number,
    { readonly at: number; readonly callback: () => void }
  >();

  now(): number {
    return this.time;
  }
  setTimeout(callback: () => void, delayMs: number): unknown {
    const id = this.nextId++;
    this.timers.set(id, { at: this.time + delayMs, callback });
    return id;
  }
  clearTimeout(handle: unknown): void {
    this.timers.delete(handle as number);
  }
  get pending(): number {
    return this.timers.size;
  }

  /** Moves time forward, running every timer that falls due, in order. */
  async advance(milliseconds: number): Promise<void> {
    const target = this.time + milliseconds;
    for (;;) {
      const due = [...this.timers.entries()]
        .filter(([, timer]) => timer.at <= target)
        .sort((a, b) => a[1].at - b[1].at)[0];
      if (!due) break;
      this.timers.delete(due[0]);
      this.time = Math.max(this.time, due[1].at);
      due[1].callback();
      await flush();
    }
    this.time = target;
  }
}

async function flush(): Promise<void> {
  for (let turn = 0; turn < 8; turn += 1) await Promise.resolve();
}

let counter = 0;
const nextId = () =>
  `00000000-0000-4000-8000-${(0xaa00 + counter++).toString(16).padStart(12, "0")}`;

function setup(
  over: {
    enabled?: () => boolean;
    blocked?: () => boolean;
    write?: (schematic: Schematic) => Promise<void>;
  } = {},
) {
  const store = new SchematicDocumentStore(dividerSchematic, nextId);
  const clock = new FakeClock();
  const writes: Schematic[] = [];
  const notices: (string | null)[] = [];
  const discards = { count: 0 };
  const port = {
    write: (schematic: Schematic) => {
      writes.push(schematic);
      return over.write ? over.write(schematic) : Promise.resolve();
    },
    discard: () => {
      discards.count += 1;
      return Promise.resolve();
    },
  };
  const autosave = new Autosave(store, port, {
    enabled: over.enabled ?? (() => true),
    blocked: over.blocked ?? (() => false),
    onNotice: (notice) => notices.push(notice),
    clock,
  });
  const stop = autosave.start();
  const edit = (y: number) => store.moveComponent(R1, { x: 4, y });
  return { store, clock, writes, notices, discards, autosave, stop, edit };
}

describe("when autosave writes", () => {
  it("does nothing while the document is clean", async () => {
    const { clock, writes } = setup();

    await clock.advance(10 * AUTOSAVE_INTERVAL_MS);

    expect(writes).toEqual([]);
  });

  it("writes two seconds after the last edit, not before", async () => {
    const { clock, writes, edit } = setup();

    edit(1);
    await clock.advance(AUTOSAVE_IDLE_MS - 1);
    expect(writes).toHaveLength(0);

    await clock.advance(1);
    expect(writes).toHaveLength(1);
  });

  it("waits for two quiet seconds: every edit starts the wait again", async () => {
    const { clock, writes, edit } = setup();

    edit(1);
    await clock.advance(1500);
    edit(2);
    await clock.advance(1500);
    edit(3);
    await clock.advance(1500);
    expect(writes).toHaveLength(0);

    await clock.advance(500);
    expect(writes).toHaveLength(1);
    // The write is of the document as it was last edited.
    expect(writes[0]!.components.find(({ id }) => id === R1)!.position.y).toBe(
      3,
    );
  });

  it("writes at most every thirty seconds while edits keep coming", async () => {
    const { clock, writes, edit } = setup();

    edit(1);
    await clock.advance(AUTOSAVE_IDLE_MS);
    expect(writes).toHaveLength(1);

    // A burst of edits right after the write: the idle wait ends, the interval has not.
    edit(2);
    await clock.advance(AUTOSAVE_IDLE_MS + 1);
    expect(writes).toHaveLength(1);

    await clock.advance(AUTOSAVE_INTERVAL_MS - AUTOSAVE_IDLE_MS - 1);
    expect(writes).toHaveLength(2);
  });

  it("does not write the same document twice", async () => {
    const { clock, writes, edit } = setup();

    edit(1);
    await clock.advance(AUTOSAVE_IDLE_MS);
    await clock.advance(5 * AUTOSAVE_INTERVAL_MS);

    expect(writes).toHaveLength(1);
  });

  it("stops writing once the document is saved, and when stopped", async () => {
    const { store, clock, writes, edit, stop } = setup();
    edit(1);
    store.markSaved();

    await clock.advance(AUTOSAVE_INTERVAL_MS);
    expect(writes).toHaveLength(0);

    edit(2);
    stop();
    await clock.advance(AUTOSAVE_INTERVAL_MS);
    expect(writes).toHaveLength(0);
    expect(clock.pending).toBe(0);
  });

  it("waits while a save or an open is running, and then writes", async () => {
    let blocked = true;
    const { clock, writes, edit, autosave } = setup({ blocked: () => blocked });

    edit(1);
    await clock.advance(10_000);
    expect(writes).toHaveLength(0);

    blocked = false;
    autosave.poke();
    await clock.advance(1_000);
    expect(writes).toHaveLength(1);
  });

  it("is off while it is not enabled, and starts when it is", async () => {
    let enabled = false;
    const { clock, writes, edit, autosave } = setup({ enabled: () => enabled });

    edit(1);
    await clock.advance(AUTOSAVE_INTERVAL_MS);
    expect(writes).toHaveLength(0);

    enabled = true;
    autosave.poke();
    await clock.advance(AUTOSAVE_IDLE_MS);
    expect(writes).toHaveLength(1);
  });

  it("never touches the project: it only calls the recovery port", async () => {
    const { clock, edit, store } = setup();
    const before = store.getState().isDirty;

    edit(1);
    await clock.advance(AUTOSAVE_INTERVAL_MS);

    // A write does not mark the document saved: the changes are still unsaved.
    expect(before).toBe(false);
    expect(store.getState().isDirty).toBe(true);
  });
});

describe("when a write fails", () => {
  it("shows one notice, keeps editing possible, retries at the next interval and clears the notice", async () => {
    let failures = 2;
    const { clock, writes, notices, edit, store } = setup({
      write: () =>
        failures-- > 0
          ? Promise.reject(new Error("disk full"))
          : Promise.resolve(),
    });

    edit(1);
    await clock.advance(AUTOSAVE_IDLE_MS);
    expect(writes).toHaveLength(1);
    expect(notices).toEqual(["Autosave is not working: disk full"]);

    // Editing goes on; the retry waits for the next interval.
    edit(2);
    await clock.advance(AUTOSAVE_INTERVAL_MS - 1);
    expect(writes).toHaveLength(1);
    await clock.advance(1);
    expect(writes).toHaveLength(2);
    expect(notices).toHaveLength(2);

    await clock.advance(AUTOSAVE_INTERVAL_MS);
    expect(writes).toHaveLength(3);
    expect(notices.at(-1)).toBeNull();
    expect(store.getState().isDirty).toBe(true);
  });

  it("states the reason of a refusal that is not an Error", async () => {
    const { clock, notices, edit } = setup({
      write: () => Promise.reject("no permission"),
    });

    edit(1);
    await clock.advance(AUTOSAVE_IDLE_MS);

    expect(notices).toEqual(["Autosave is not working: no permission"]);
  });
});

describe("discarding the recovery file", () => {
  it("waits for the write in flight, so a late write cannot bring the file back", async () => {
    let finish: () => void = () => {};
    const order: string[] = [];
    const { clock, edit, autosave, writes } = setup({
      write: () =>
        new Promise<void>((resolve) => {
          finish = () => {
            order.push("write finished");
            resolve();
          };
        }),
    });
    edit(1);
    await clock.advance(AUTOSAVE_IDLE_MS);
    expect(writes).toHaveLength(1);

    let discarded = false;
    const discarding = autosave.discard().then(() => {
      discarded = true;
      order.push("discarded");
    });
    await flush();
    expect(discarded).toBe(false);

    finish();
    await discarding;

    expect(order).toEqual(["write finished", "discarded"]);
  });

  it("asks the port to delete the file, and forgets what was written", async () => {
    const { clock, edit, autosave, discards, writes } = setup();
    edit(1);
    await clock.advance(AUTOSAVE_IDLE_MS);

    await autosave.discard();

    expect(discards.count).toBe(1);
    expect(writes).toHaveLength(1);
  });
});
