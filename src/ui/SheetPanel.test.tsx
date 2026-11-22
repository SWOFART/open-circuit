import { Children, isValidElement, type ReactNode } from "react";
import { describe, expect, it, vi } from "vitest";
import { SchematicDocumentStore } from "../store/document";
import { dividerSchematic } from "../store/fixtures";
import { SheetPanel } from "./SheetPanel";

vi.mock("react", async (importOriginal) => {
  const react = await importOriginal<typeof import("react")>();
  return {
    ...react,
    useEffect: vi.fn(),
    useState: (initial: unknown) => [initial, vi.fn()],
  };
});

function clickHandler(
  node: ReactNode,
  caption: string,
): (() => void) | undefined {
  for (const child of Children.toArray(node)) {
    if (!isValidElement<{ children?: ReactNode; onClick?: () => void }>(child))
      continue;
    if (child.type === "button" && child.props.children === caption) {
      return child.props.onClick;
    }
    const nested = clickHandler(child.props.children, caption);
    if (nested) return nested;
  }
}

describe("sheet navigation", () => {
  it("keeps the default active sheet without moving the viewport when reordering", () => {
    const store = new SchematicDocumentStore(dividerSchematic);
    const power = store.saveSheet({
      name: "Power",
      size: "a4",
      origin: { x: 0, y: 0 },
    });
    const control = store.saveSheet({
      name: "Control",
      size: "a4",
      origin: { x: 59, y: 0 },
    });
    const schematic = store.getState().schematic;
    const onSelect = vi.fn();
    const panel = SheetPanel({
      schematic,
      store,
      selected: schematic.documentation.sheets[0],
      disabled: false,
      onSelect,
    });
    const moveLater = clickHandler(panel, "Move sheet later");
    expect(moveLater).toBeDefined();
    moveLater?.();
    expect(
      store.getState().schematic.documentation.sheets.map(({ id }) => id),
    ).toEqual([control, power]);
    expect(onSelect).toHaveBeenCalledExactlyOnceWith(
      expect.objectContaining({ id: power }),
      false,
    );
  });
});
