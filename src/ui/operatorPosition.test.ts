import { describe, expect, it } from "vitest";
import { operatorPosition } from "./operatorPosition";

describe("Stage 14 operator commands", () => {
  const componentId = "00000000-0000-4000-8000-000000001411";
  it("maps selector actions and limits them to selectors", () => {
    const selector = {
      kind: "selectorSwitch",
      componentId,
      position: "center",
    } as const;
    expect(operatorPosition("left", selector)).toBe("left");
    expect(operatorPosition("center", selector)).toBe("center");
    expect(operatorPosition("right", selector)).toBe("right");
    expect(operatorPosition("press", selector)).toBeNull();
  });

  it("sends an explicit emergency reset and leaves release as a no-op", () => {
    const stop = {
      kind: "emergencyStop",
      componentId,
      position: "latched",
    } as const;
    expect(operatorPosition("press", stop)).toBe("pressed");
    expect(operatorPosition("reset", stop)).toBe("reset");
    expect(operatorPosition("release", stop)).toBeNull();
  });

  it("uses momentary limit actions and toggles a key switch", () => {
    const limit = {
      kind: "limitSwitch",
      componentId,
      position: "released",
    } as const;
    const key = { kind: "keySwitch", componentId, position: "on" } as const;
    expect(operatorPosition("press", limit)).toBe("pressed");
    expect(operatorPosition("release", limit)).toBe("released");
    expect(operatorPosition("toggle", key)).toBe("open");
  });
});
