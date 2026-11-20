import { execFileSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import { expect, it } from "vitest";

it("checks committed Stage 10 references without running the Engine or ngspice", () => {
  const script = fileURLToPath(
    new URL("./stage10-references.mjs", import.meta.url),
  );
  const output = execFileSync(process.execPath, [script, "--check"], {
    encoding: "utf8",
  });
  expect(output).toMatch(/Verified 26 Stage 10 reference circuits/);
});
