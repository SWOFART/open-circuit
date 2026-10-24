import { describe, expect, it } from "vitest";
import expected from "../../tests/golden/stage-1-divider.expected.json";
import { DISPLAY_SIGNIFICANT_DIGITS, formatSi, parseSi } from "./units";

// docs/contracts/stage-1.md section 6 fixes the display rounding and the acceptance
// tolerances. This test ties them together: whatever the UI shows for the fixture must
// still be within the tolerances of the exact value, so rounding cannot hide an error
// or create one.
const VOLTAGE_TOLERANCE_VOLTS = 1e-6;
const CURRENT_TOLERANCE_AMPS = 1e-9;

function shown(value: number, unit: "volt" | "amp"): number {
  const parsed = parseSi(formatSi(value, unit), unit);

  if (!parsed.ok) {
    throw new Error(`The displayed value for ${value} cannot be read back.`);
  }

  return parsed.value;
}

describe("display rounding of the Stage 1 acceptance fixture", () => {
  it("uses seven significant digits", () => {
    expect(DISPLAY_SIGNIFICANT_DIGITS).toBe(7);
  });

  it.each(Object.entries(expected))(
    "keeps every displayed value of %s within the acceptance tolerances",
    (_name, values) => {
      expect(
        Math.abs(
          shown(values.resistor1VoltageVolts, "volt") -
            values.resistor1VoltageVolts,
        ),
      ).toBeLessThanOrEqual(VOLTAGE_TOLERANCE_VOLTS);
      expect(
        Math.abs(
          shown(values.resistor2VoltageVolts, "volt") -
            values.resistor2VoltageVolts,
        ),
      ).toBeLessThanOrEqual(VOLTAGE_TOLERANCE_VOLTS);
      expect(
        Math.abs(
          shown(values.sourceCurrentAmps, "amp") - values.sourceCurrentAmps,
        ),
      ).toBeLessThanOrEqual(CURRENT_TOLERANCE_AMPS);
    },
  );

  it("shows Vout as the issue states it", () => {
    expect(
      formatSi(expected.equalResistors.resistor2VoltageVolts, "volt"),
    ).toBe("2.499875 V");
    expect(
      formatSi(expected.resistor1At3000Ohms.resistor2VoltageVolts, "volt"),
    ).toBe("1.249969 V");
  });

  it("would not have met the tolerance with six digits", () => {
    // Documents why seven are needed: six digits show 2.49988 V, 5 µV off.
    const sixDigits = Number(
      expected.equalResistors.resistor2VoltageVolts.toPrecision(6),
    );

    expect(
      Math.abs(sixDigits - expected.equalResistors.resistor2VoltageVolts),
    ).toBeGreaterThan(VOLTAGE_TOLERANCE_VOLTS);
  });
});
