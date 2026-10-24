import { existsSync, readdirSync, readFileSync } from "node:fs";
import { join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

// The production bundle must never contain the development mock adapter: it holds
// predetermined readings that must not reach a user. The mock is loaded only through a
// dynamic import behind `import.meta.env.DEV`, which the production build removes; this
// check proves that, on the bundle that is actually shipped.

const repositoryRoot = fileURLToPath(new URL("../", import.meta.url));
const distDirectory = join(repositoryRoot, "dist");

/** String literals that exist only in the mock adapter and its development banner. */
export const mockMarkers = [
  "No predetermined response",
  "Development mock",
  "00000000-0000-4000-8000-0000000000a0",
];

/**
 * Text that must be in a real bundle. It belongs to the production adapter, so finding
 * it shows that the scan read the application code and did not pass on an empty or
 * unrelated directory.
 */
export const requiredMarkers = [
  "The Rust engine is only available in the desktop app.",
];

export function findMockMarkers(source) {
  return mockMarkers.filter((marker) => source.includes(marker));
}

export function missingRequiredMarkers(source) {
  return requiredMarkers.filter((marker) => !source.includes(marker));
}

function scriptFiles(directory) {
  return readdirSync(directory, { withFileTypes: true }).flatMap((entry) => {
    const path = join(directory, entry.name);

    if (entry.isDirectory()) {
      return scriptFiles(path);
    }

    return entry.isFile() && /\.m?js$/.test(entry.name) ? [path] : [];
  });
}

function main() {
  if (!existsSync(distDirectory)) {
    console.error(
      "dist/ does not exist. Build the frontend first (pnpm build or pnpm build:web).",
    );
    process.exit(1);
  }

  const files = scriptFiles(distDirectory);
  const source = files.map((file) => readFileSync(file, "utf8")).join("\n");
  const missing = missingRequiredMarkers(source);

  if (files.length === 0 || missing.length > 0) {
    console.error(
      "The production bundle looks empty or unexpected, so it cannot be checked.",
    );
    for (const marker of missing) {
      console.error(`- missing expected text: ${marker}`);
    }
    process.exit(1);
  }

  const found = findMockMarkers(source);

  if (found.length > 0) {
    console.error(
      "The production bundle contains the development mock adapter:",
    );
    for (const marker of found) {
      console.error(`- ${marker}`);
    }
    process.exit(1);
  }

  console.log(
    `The production bundle (${files.length} scripts) contains no mock adapter.`,
  );
}

if (
  process.argv[1] &&
  resolve(process.argv[1]) === fileURLToPath(import.meta.url)
) {
  main();
}
