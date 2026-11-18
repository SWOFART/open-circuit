import { execFileSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import { expect, it } from "vitest";

it("checked-in Stage 9 references match the offline generator without writes", () => {
  const output = execFileSync(
    process.execPath,
    [
      fileURLToPath(new URL("./stage9-references.mjs", import.meta.url)),
      "--check",
    ],
    { encoding: "utf8" },
  );
  expect(output).toContain(
    "Verified 27 Stage 9 circuits, 1460 stored observations",
  );
});
