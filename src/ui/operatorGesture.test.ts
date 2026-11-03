import { describe, expect, it } from "vitest";
import { HeldPress, pushButtonKey, switchKey } from "./operatorGesture";
import { operatorPosition } from "./operatorPosition";

describe("a held push button", () => {
  function held() {
    const sent: string[] = [];
    return { press: new HeldPress((action) => sent.push(action)), sent };
  }

  it("presses once and releases once, however often it is told", () => {
    const { press, sent } = held();

    press.press();
    press.press();
    press.release();
    press.release();

    expect(sent).toEqual(["press", "release"]);
    expect(press.isHeld).toBe(false);
  });

  it("never releases a button it did not press", () => {
    const { press, sent } = held();

    press.release();

    expect(sent).toEqual([]);
  });

  it("can be pressed again after a release, and forgets a press it can no longer release", () => {
    const { press, sent } = held();

    press.press();
    press.release();
    press.press();
    press.forget();
    press.release();

    expect(sent).toEqual(["press", "release", "press"]);
  });
});

describe("keys on a focused push button", () => {
  it("presses on a key down of Space or Enter and releases on the key up", () => {
    for (const key of [" ", "Enter"]) {
      expect(pushButtonKey("keydown", key, false)).toEqual({
        action: "press",
        swallow: true,
      });
      expect(pushButtonKey("keyup", key, false)).toEqual({
        action: "release",
        swallow: true,
      });
    }
  });

  it("ignores the repeats of a held key, so a held key does not press again", () => {
    expect(pushButtonKey("keydown", " ", true)).toEqual({
      action: null,
      swallow: true,
    });
  });

  it("releases on Escape and leaves every other key alone", () => {
    expect(pushButtonKey("keydown", "Escape", false)).toEqual({
      action: "release",
      swallow: false,
    });
    expect(pushButtonKey("keydown", "a", false)).toBeNull();
    expect(pushButtonKey("keyup", "Tab", false)).toBeNull();
  });
});

describe("keys on a focused switch", () => {
  it("toggles once on a key down of Space or Enter and swallows the key up", () => {
    for (const key of [" ", "Enter"]) {
      expect(switchKey("keydown", key, false)).toEqual({
        toggle: true,
        swallow: true,
      });
      expect(switchKey("keyup", key, false)).toEqual({
        toggle: false,
        swallow: true,
      });
    }
  });

  it("does not toggle again on the repeats of a held key, and swallows them", () => {
    for (const key of [" ", "Enter"]) {
      expect(switchKey("keydown", key, true)).toEqual({
        toggle: false,
        swallow: true,
      });
    }
  });

  it("leaves every other key alone", () => {
    expect(switchKey("keydown", "Tab", false)).toBeNull();
    expect(switchKey("keydown", "Escape", false)).toBeNull();
  });
});

describe("the position a user action asks for", () => {
  const id = "00000000-0000-4000-8000-0000000000aa";

  it("toggles a switch to the other position the Engine reported", () => {
    expect(
      operatorPosition("toggle", {
        kind: "switch",
        componentId: id,
        position: "open",
        conducting: false,
      }),
    ).toBe("closed");
    expect(
      operatorPosition("toggle", {
        kind: "switch",
        componentId: id,
        position: "closed",
        conducting: true,
      }),
    ).toBe("open");
  });

  it("presses and releases a push button", () => {
    const button = {
      kind: "pushButton",
      componentId: id,
      position: "released",
      conducting: false,
    } as const;

    expect(operatorPosition("press", button)).toBe("pressed");
    expect(operatorPosition("release", button)).toBe("released");
  });

  it("asks for nothing when there is no state to go by or the device is another kind", () => {
    const relay = {
      kind: "relay",
      componentId: id,
      state: "released",
    } as const;

    expect(operatorPosition("toggle", undefined)).toBeNull();
    expect(operatorPosition("press", undefined)).toBeNull();
    expect(operatorPosition("toggle", relay)).toBeNull();
    expect(operatorPosition("press", relay)).toBeNull();
    expect(
      operatorPosition("press", {
        kind: "switch",
        componentId: id,
        position: "open",
        conducting: false,
      }),
    ).toBeNull();
  });
});
