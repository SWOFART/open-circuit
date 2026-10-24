import {
  existsSync,
  mkdtempSync,
  readdirSync,
  readFileSync,
  rmSync,
} from "node:fs";
import { join, relative } from "node:path";
import { tmpdir } from "node:os";
import { fileURLToPath } from "node:url";
import { spawnSync } from "node:child_process";

const repositoryRoot = fileURLToPath(new URL("../", import.meta.url));
const checkedInDirectory = join(repositoryRoot, "src", "generated");
const cargoCommand = process.platform === "win32" ? "cargo.exe" : "cargo";

function snapshot(directory, relativeTo) {
  const files = new Map();

  function visit(currentDirectory) {
    if (!existsSync(currentDirectory)) {
      return;
    }

    for (const entry of readdirSync(currentDirectory, {
      withFileTypes: true,
    })) {
      const path = join(currentDirectory, entry.name);

      if (entry.isDirectory()) {
        visit(path);
      } else if (entry.isFile()) {
        files.set(relative(relativeTo, path), readFileSync(path));
      }
    }
  }

  visit(directory);
  return files;
}

function snapshotsMatch(expected, actual) {
  if (expected.size !== actual.size) {
    return false;
  }

  for (const [path, contents] of expected) {
    const actualContents = actual.get(path);

    if (!actualContents || !contents.equals(actualContents)) {
      return false;
    }
  }

  return true;
}

const generatedDirectory = mkdtempSync(join(tmpdir(), "open-circuit-types-"));
let exitCode = 0;

try {
  const generation = spawnSync(
    cargoCommand,
    ["test", "-p", "open-circuit-engine"],
    {
      cwd: repositoryRoot,
      env: {
        ...process.env,
        TS_RS_EXPORT_DIR: generatedDirectory,
      },
      stdio: "inherit",
    },
  );

  if (generation.error || generation.status !== 0) {
    console.error(generation.error?.message ?? "Type generation failed.");
    exitCode = 1;
  } else {
    const expected = snapshot(generatedDirectory, generatedDirectory);
    const actual = snapshot(checkedInDirectory, checkedInDirectory);

    if (!snapshotsMatch(expected, actual)) {
      console.error(
        "Generated TypeScript bindings are out of date. Run pnpm types:generate and review src/generated.",
      );
      exitCode = 1;
    } else {
      console.log("Generated TypeScript bindings are up to date.");
    }
  }
} finally {
  rmSync(generatedDirectory, { force: true, recursive: true });
}

process.exitCode = exitCode;
