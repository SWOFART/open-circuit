import { execFileSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import { expect, it } from "vitest";

it("checks committed Stage 14 references offline", () => {
  const script = fileURLToPath(
    new URL("./stage14-references.mjs", import.meta.url),
  );
  const output = execFileSync(process.execPath, [script, "--check"], {
    encoding: "utf8",
  });
  expect(output).toMatch(/Verified 72 Stage 14 reference cases/);
});
