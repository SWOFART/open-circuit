// Stage 12 (docs/contracts/stage-12-cabinet-documentation.md section 2): a live schematic is
// schema version 3 and carries an empty `documentation` block after its rails. The reference
// generators describe their circuits in the earlier shape, so every schematic they write goes
// through this function, which keeps the committed fixtures and the generators in step.

const emptyDocumentation = () => ({
  designators: [],
  wireNumbers: [],
  terminals: [],
  notes: [],
  titleBlock: {
    title: "",
    drawingNumber: "",
    revision: "",
    author: "",
    company: "",
    date: "",
  },
  sheets: [],
});

export function withDocumentation(value) {
  if (Array.isArray(value)) return value.map(withDocumentation);
  if (value === null || typeof value !== "object") return value;

  const isLiveSchematic =
    value.schemaVersion === 2 && "rails" in value && "components" in value;
  const result = {};

  for (const [key, child] of Object.entries(value)) {
    result[key] = withDocumentation(child);
    if (isLiveSchematic && key === "rails") {
      result.documentation = emptyDocumentation();
    }
  }
  if (isLiveSchematic) result.schemaVersion = 3;

  return result;
}

/** The inverse, for a generator that reads committed fixtures and describes them in the earlier shape. */
export function withoutDocumentation(value) {
  if (Array.isArray(value)) return value.map(withoutDocumentation);
  if (value === null || typeof value !== "object") return value;

  const isLiveSchematic =
    value.schemaVersion === 3 &&
    "documentation" in value &&
    "components" in value;
  const result = {};

  for (const [key, child] of Object.entries(value)) {
    if (isLiveSchematic && key === "documentation") continue;
    result[key] = withoutDocumentation(child);
  }
  if (isLiveSchematic) result.schemaVersion = 2;

  return result;
}
