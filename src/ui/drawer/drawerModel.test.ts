import { describe, expect, it } from "vitest";
import catalogFixture from "../../../tests/golden/stage-1-catalog.json";
import type { ComponentCatalog } from "../../generated/ComponentCatalog";
import { allowedCatalog } from "../../store/studentAssignment";
import { buildDrawerView, moveHighlight, searchKeyAction } from "./drawerModel";
import { DEFAULT_DRAWER_PREFERENCES } from "./drawerPreferences";

const catalog = catalogFixture as ComponentCatalog;
const placeable = () => true;
const ids = (entries: readonly { typeId: string }[] | undefined) =>
  entries?.map((entry) => entry.typeId);

describe("the drawer list", () => {
  it("groups the parts the editor can draw by the catalog categories, in order", () => {
    const view = buildDrawerView(
      catalog,
      DEFAULT_DRAWER_PREFERENCES,
      "",
      placeable,
    );
    const shown = ids(view.items);

    expect(view.results).toBeNull();
    expect(view.groups.map((group) => group.id)).toEqual(catalog.categories);
    // Every catalog part is listed; parts without an editor symbol show a placeholder.
    expect(shown).toContain("ac_voltage_source");
    expect(shown).toContain("resistor");
    expect(
      ids(view.groups.find((group) => group.id === "switching")?.entries),
    ).toEqual([
      "relay_spdt",
      "switch_spst",
      "push_button_no",
      "push_button_nc",
      "limit_switch",
      "emergency_stop",
      "selector_switch",
      "key_switch",
      "relay_on_delay",
      "relay_off_delay",
      "contactor",
      "contactor_three_pole",
    ]);
  });

  it("shows Favorites and Recent above the categories, only when they are not empty", () => {
    const view = buildDrawerView(
      catalog,
      {
        ...DEFAULT_DRAWER_PREFERENCES,
        favorites: ["fuse"],
        recent: ["diode", "fuse"],
      },
      "",
      placeable,
    );

    expect(view.groups.slice(0, 2).map((group) => group.id)).toEqual([
      "favorites",
      "recent",
    ]);
    expect(ids(view.groups[1]?.entries)).toEqual(["diode", "fuse"]);
    // A part in Favorites also appears under its category.
    expect(
      ids(view.groups.find((group) => group.id === "protection")?.entries),
    ).toEqual([
      "fuse",
      "breaker_three_pole",
      "thermal_overload_three_pole",
      "earth_electrode",
      "rcd_two_pole",
      "rcd_four_pole",
    ]);
    expect(
      buildDrawerView(catalog, DEFAULT_DRAWER_PREFERENCES, "", placeable)
        .groups[0]?.id,
    ).toBe("sources");
  });

  it("shows ranked results without groups for a non-empty search", () => {
    const view = buildDrawerView(
      catalog,
      DEFAULT_DRAWER_PREFERENCES,
      "motor",
      placeable,
    );

    expect(view.groups).toEqual([]);
    expect(ids(view.results ?? undefined)).toEqual([
      "pmdc_motor",
      "induction_motor_three_phase",
      "thermal_overload_three_pole",
      "transformer_three_phase",
      "three_phase_load_star",
      "three_phase_load_delta",
    ]);
    expect(view.items).toEqual(view.results);
  });

  it("offers only the allowed types in an assignment and keeps the preferences", () => {
    const limited = allowedCatalog(catalog, {
      allowedTypes: ["resistor", "dc_voltage_source"],
      locked: new Set(),
      findFault: false,
    });
    const preferences = {
      ...DEFAULT_DRAWER_PREFERENCES,
      favorites: ["fuse", "resistor"] as const,
      recent: ["diode"] as const,
    };

    const view = buildDrawerView(limited, preferences, "", placeable);

    expect(new Set(ids(view.items))).toEqual(
      new Set(["resistor", "dc_voltage_source"]),
    );
    expect(view.groups.map((group) => group.id)).toEqual([
      "favorites",
      "sources",
      "passive",
    ]);
    // The search runs over the allowed list only.
    expect(
      buildDrawerView(limited, preferences, "fuse", placeable).results,
    ).toEqual([]);
    // The entries that are not allowed stay stored, so they return outside the assignment.
    expect(preferences.favorites).toEqual(["fuse", "resistor"]);
  });

  it("is empty before the catalog loads", () => {
    const view = buildDrawerView(
      null,
      DEFAULT_DRAWER_PREFERENCES,
      "",
      placeable,
    );

    expect(view.items).toEqual([]);
  });
});

describe("keyboard navigation", () => {
  it("moves the highlight with Down, Up, Home and End and stops at the ends", () => {
    expect(moveHighlight(-1, 3, "ArrowDown")).toBe(0);
    expect(moveHighlight(0, 3, "ArrowDown")).toBe(1);
    expect(moveHighlight(2, 3, "ArrowDown")).toBe(2);
    expect(moveHighlight(2, 3, "ArrowUp")).toBe(1);
    expect(moveHighlight(0, 3, "ArrowUp")).toBe(0);
    expect(moveHighlight(1, 3, "Home")).toBe(0);
    expect(moveHighlight(1, 3, "End")).toBe(2);
    expect(moveHighlight(0, 0, "ArrowDown")).toBe(-1);
  });

  const action = (
    key: string,
    extra: Partial<Parameters<typeof searchKeyAction>[0]> = {},
  ) =>
    searchKeyAction({
      key,
      shiftKey: false,
      highlight: -1,
      count: 4,
      query: "",
      ...extra,
    });

  it("chooses the highlighted part on Enter and places it on Shift+Enter", () => {
    expect(action("Enter", { highlight: 2 })).toEqual({
      kind: "choose",
      index: 2,
      placeAtCenter: false,
    });
    expect(action("Enter", { highlight: 2, shiftKey: true })).toEqual({
      kind: "choose",
      index: 2,
      placeAtCenter: true,
    });
  });

  it("chooses the best match on Enter after typing, and nothing with no highlight and no query", () => {
    expect(action("Enter", { query: "fu" })).toEqual({
      kind: "choose",
      index: 0,
      placeAtCenter: false,
    });
    expect(action("Enter")).toEqual({ kind: "none" });
    expect(action("Enter", { query: "zzz", count: 0 })).toEqual({
      kind: "none",
    });
  });

  it("clears a query on Escape and closes the drawer in an empty search", () => {
    expect(action("Escape", { query: "fu" })).toEqual({ kind: "clear" });
    expect(action("Escape")).toEqual({ kind: "close" });
  });

  it("leaves Home and End to the text field until the list is walked", () => {
    expect(action("Home", { query: "fu" })).toEqual({ kind: "none" });
    expect(action("End", { query: "fu", highlight: 1 })).toEqual({
      kind: "highlight",
      index: 3,
    });
    expect(action("Home")).toEqual({ kind: "highlight", index: 0 });
  });

  it("ignores other keys", () => {
    expect(action("a")).toEqual({ kind: "none" });
  });
});
