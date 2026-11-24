import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";

const repositoryRoot = fileURLToPath(new URL("../", import.meta.url));
const cargoCommand = process.platform === "win32" ? "cargo.exe" : "cargo";
const args = ["test", "-p", "open-circuit-engine", ...process.argv.slice(2)];
const result = spawnSync(cargoCommand, args, {
  cwd: repositoryRoot,
  env: {
    ...process.env,
    TS_RS_EXPORT_DIR: fileURLToPath(
      new URL("../src/generated/", import.meta.url),
    ),
  },
  stdio: "inherit",
});

if (result.error) {
  console.error(`Failed to run cargo: ${result.error.message}`);
  process.exit(1);
}

process.exit(result.status ?? 1);
