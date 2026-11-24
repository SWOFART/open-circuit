import { execFileSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { expect, it } from "vitest";

const script = fileURLToPath(
  new URL("./stage13-references.mjs", import.meta.url),
);
const golden = (name) =>
  JSON.parse(
    readFileSync(
      fileURLToPath(
        new URL(`../tests/golden/stage-13/${name}`, import.meta.url),
      ),
      "utf8",
    ),
  );

it("checks the committed Stage 13 references without running the Engine", () => {
  const output = execFileSync(process.execPath, [script, "--check"], {
    encoding: "utf8",
  });
  expect(output).toMatch(/Verified 49 Stage 13 reference circuits/);
});

it("pairs every wrong variant with a correct fixture that expects something else", () => {
  const { fixtures } = golden("analytical.json");
  const byId = new Map(fixtures.map((fixture) => [fixture.id, fixture]));

  for (const wrong of fixtures.filter((f) => f.variant === "wrong")) {
    const original = byId.get(wrong.wrongOf);
    expect(original?.variant, wrong.id).toBe("correct");
    expect(wrong.request, wrong.id).not.toEqual(original.request);
    expect(wrong.checks, wrong.id).not.toEqual(original.checks);
  }
});

it("records a hash of the analytical data that the data really has", async () => {
  const { createHash } = await import("node:crypto");
  const text = readFileSync(
    fileURLToPath(
      new URL("../tests/golden/stage-13/analytical.json", import.meta.url),
    ),
    "utf8",
  );

  expect(golden("manifest.json").analyticalSha256).toBe(
    createHash("sha256").update(text).digest("hex"),
  );
});

it("keeps the closed forms independent of the Engine and of any package", () => {
  const source = readFileSync(script, "utf8");
  const imports = [...source.matchAll(/from "([^"]+)"/g)].map((m) => m[1]);

  expect(imports.length).toBeGreaterThan(0);
  for (const specifier of imports) {
    expect(specifier).toMatch(/^(node:|\.\/documentation-defaults\.mjs$)/);
  }
  expect(source).not.toMatch(/open-circuit-engine/);
});
