import { spawnSync } from "node:child_process";
import { readdirSync, readFileSync } from "node:fs";
import { join, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const repositoryRoot = fileURLToPath(new URL("../", import.meta.url));
const cargoCommand = process.platform === "win32" ? "cargo.exe" : "cargo";

// Engine crates must stay independent of Tauri and any UI/webview runtime.
const enginePackages = ["open-circuit-engine", "open-circuit-engine-cli"];
const forbiddenCrates = new Set([
  "open-circuit-app",
  "tauri",
  "tao",
  "webkit2gtk",
  "webview2-com",
  "wry",
]);

// EditorCore is plain TypeScript/PixiJS and must not import React.
const editorDirectory = join(repositoryRoot, "src", "editor");
const sourceExtensions = /\.(?:[cm]?[jt]s|[jt]sx)$/;
const reactImportPattern =
  /(?:\bfrom\s*|\bimport\s*\(?\s*|\brequire\s*\(\s*)["'](?:react|react-dom)(?:\/[^"']*)?["']/;

export function forbiddenCrateNames(cargoTreeOutput) {
  const names = new Set(
    cargoTreeOutput
      .split(/\r?\n/)
      .map((line) => line.trim().split(/\s+/)[0])
      .filter(Boolean),
  );

  return [...names]
    .filter((name) => forbiddenCrates.has(name) || name.startsWith("tauri-"))
    .sort();
}

export function importsReact(source) {
  return reactImportPattern.test(source);
}

function sourceFiles(directory) {
  return readdirSync(directory, { withFileTypes: true }).flatMap((entry) => {
    const path = join(directory, entry.name);

    if (entry.isDirectory()) {
      return sourceFiles(path);
    }

    return entry.isFile() && sourceExtensions.test(entry.name) ? [path] : [];
  });
}

function checkEngineDependencies() {
  const violations = [];

  for (const packageName of enginePackages) {
    // --target all includes platform-specific dependencies, so a Windows-only
    // or Linux-only Tauri dependency cannot slip past a single-OS run.
    const result = spawnSync(
      cargoCommand,
      [
        "tree",
        "--locked",
        "--package",
        packageName,
        "--target",
        "all",
        "--edges",
        "normal,build,dev",
        "--prefix",
        "none",
      ],
      { cwd: repositoryRoot, encoding: "utf8" },
    );

    if (result.error || result.status !== 0) {
      const reason = result.error?.message ?? result.stderr;
      throw new Error(`cargo tree failed for ${packageName}: ${reason}`);
    }

    for (const crate of forbiddenCrateNames(result.stdout)) {
      violations.push(`${packageName} depends on ${crate}`);
    }
  }

  return violations;
}

function checkEditorImports() {
  return sourceFiles(editorDirectory)
    .filter((file) => importsReact(readFileSync(file, "utf8")))
    .map((file) => {
      const path = relative(repositoryRoot, file).split("\\").join("/");
      return `${path} imports React`;
    });
}

function main() {
  const violations = [...checkEngineDependencies(), ...checkEditorImports()];

  if (violations.length > 0) {
    console.error("Architecture dependency boundaries violated:");
    for (const violation of violations) {
      console.error(`- ${violation}`);
    }
    process.exit(1);
  }

  console.log(
    "Architecture dependency boundaries hold: the engine crates have no Tauri or UI dependencies, and src/editor does not import React.",
  );
}

if (
  process.argv[1] &&
  resolve(process.argv[1]) === fileURLToPath(import.meta.url)
) {
  main();
}
