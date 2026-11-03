import type { OperateAction } from "../editor/interaction";

/**
 * Remembers whether a control has pressed a push button and not yet released it, so that
 * every release request matches a press, and a press is never repeated while it is held.
 * It is not a position: the position shown is always the one the Engine reported.
 */
export class HeldPress {
  private held = false;

  constructor(private send: (action: "press" | "release") => void) {}

  get isHeld(): boolean {
    return this.held;
  }

  /** Where the requests go; changes with the latest handler. */
  setSender(send: (action: "press" | "release") => void): void {
    this.send = send;
  }

  press(): void {
    if (!this.held) {
      this.held = true;
      this.send("press");
    }
  }

  release(): void {
    if (this.held) {
      this.held = false;
      this.send("release");
    }
  }

  /** The control can no longer send anything, so there is nothing left to release. */
  forget(): void {
    this.held = false;
  }
}

/**
 * What a key event does to a focused switch (docs/contracts/stage-6-control-circuits.md
 * section 6.3): Space or Enter toggles it once, on key down, and the repeats of a held key
 * do not toggle it again. `swallow` is true for both key events of those keys, so that the
 * browser's own click, which repeats while Enter is held, never reaches the switch. It is
 * null for every other key.
 */
export function switchKey(
  type: "keydown" | "keyup",
  key: string,
  repeat: boolean,
): { readonly toggle: boolean; readonly swallow: true } | null {
  if (key !== " " && key !== "Enter") {
    return null;
  }
  return { toggle: type === "keydown" && !repeat, swallow: true };
}

/**
 * What a key event does to a focused push button (docs/contracts/
 * stage-6-control-circuits.md section 6.3): Space or Enter presses on key down, ignoring
 * the repeats of a held key, and releases on key up; Escape releases. `swallow` means the
 * default action, a click, must be prevented. It is null for every other key.
 */
export function pushButtonKey(
  type: "keydown" | "keyup",
  key: string,
  repeat: boolean,
): { readonly action: OperateAction | null; readonly swallow: boolean } | null {
  if (type === "keydown" && key === "Escape") {
    return { action: "release", swallow: false };
  }
  if (key !== " " && key !== "Enter") {
    return null;
  }
  if (type === "keyup") {
    return { action: "release", swallow: true };
  }
  return { action: repeat ? null : "press", swallow: true };
}
