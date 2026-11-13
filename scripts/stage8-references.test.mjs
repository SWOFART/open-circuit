import { execFileSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import { expect, it } from "vitest";

it("checked-in Stage 8 data matches the deliberate offline generator without writes", () => {
  const output = execFileSync(
    process.execPath,
    [
      fileURLToPath(new URL("./stage8-references.mjs", import.meta.url)),
      "--check",
    ],
    { encoding: "utf8" },
  );
  expect(output).toContain("Verified 6 circuit pairs and 27 topology cases");
});
