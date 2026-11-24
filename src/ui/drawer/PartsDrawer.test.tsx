import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import catalogFixture from "../../../tests/golden/stage-1-catalog.json";
import type { ComponentCatalog } from "../../generated/ComponentCatalog";
import { DEFAULT_DRAWER_PREFERENCES } from "./drawerPreferences";
import { PartsDrawer, type PartsDrawerProps } from "./PartsDrawer";

const catalog = catalogFixture as ComponentCatalog;
const noop = () => {};

function render(props: Partial<PartsDrawerProps> = {}) {
  return renderToStaticMarkup(
    <PartsDrawer
      catalog={catalog}
      preferences={DEFAULT_DRAWER_PREFERENCES}
      onPreferencesChange={noop}
      onChoose={noop}
      onPlaceAtCenter={noop}
      onDrop={noop}
      onClose={noop}
      {...props}
    />,
  );
}

describe("the parts drawer", () => {
  it("is a labelled region with a labelled search field and a listbox", () => {
    const markup = render();

    expect(markup).toContain('aria-label="Parts"');
    expect(markup).toContain('for="parts-search"');
    expect(markup).toContain(">Search parts<");
    expect(markup).toContain('role="combobox"');
    expect(markup).toContain('aria-controls="parts-drawer-list"');
    expect(markup).toContain('role="listbox"');
  });

  it("lists the parts under category headings, as options", () => {
    const markup = render();

    for (const heading of [
      "Sources",
      "Passive",
      "Protection",
      "Switching and control",
      "Measurement",
      "Motors and loads",
      "Semiconductors",
    ]) {
      expect(markup).toContain(`>${heading}<`);
    }
    for (const name of [
      "Resistor",
      "DC voltage source",
      "Contactor",
      "Diode",
    ]) {
      expect(markup).toContain(`>${name}</span>`);
    }
    expect(markup.match(/role="option"/g)).toHaveLength(
      catalog.componentTypes.length,
    );
    expect(markup).not.toContain("component.diode.name");
  });

  it("has a favorite toggle with an accessible name for every part", () => {
    expect(render()).toContain('aria-label="Add Fuse to favorites"');
    expect(
      render({
        preferences: { ...DEFAULT_DRAWER_PREFERENCES, favorites: ["fuse"] },
      }),
    ).toContain('aria-label="Remove Fuse from favorites"');
  });

  it("shows Favorites and Recent groups above the categories", () => {
    const markup = render({
      preferences: {
        ...DEFAULT_DRAWER_PREFERENCES,
        favorites: ["fuse"],
        recent: ["diode"],
      },
    });

    expect(markup.indexOf(">Favorites<")).toBeGreaterThan(-1);
    expect(markup.indexOf(">Favorites<")).toBeLessThan(
      markup.indexOf(">Recent<"),
    );
    expect(markup.indexOf(">Recent<")).toBeLessThan(
      markup.indexOf(">Sources<"),
    );
  });

  it("shows ranked, ungrouped results and their count for a search", () => {
    const markup = render({ initialQuery: "coil" });

    expect(markup).toContain("3 parts found.");
    expect(markup).not.toContain(">Sources<");
    expect(markup.indexOf(">Inductor<")).toBeLessThan(
      markup.indexOf(">SPDT relay<"),
    );
  });

  it("says so when nothing matches, with the query", () => {
    expect(render({ initialQuery: "zzz" })).toContain("No part matches “zzz”.");
  });

  it("offers only the parts of the catalog it is given", () => {
    const limited: ComponentCatalog = {
      ...catalog,
      componentTypes: catalog.componentTypes.filter(
        (entry) => entry.typeId === "resistor",
      ),
    };
    const markup = render({ catalog: limited });

    expect(markup.match(/role="option"/g)).toHaveLength(1);
    expect(markup).not.toContain(">Sources<");
    expect(markup).not.toContain(">Fuse<");
  });

  it("reflects the docked state and the stored width", () => {
    const docked = render({
      preferences: { ...DEFAULT_DRAWER_PREFERENCES, docked: true, width: 360 },
    });

    expect(docked).toContain("parts-drawer-docked");
    expect(docked).toContain("width:360px");
    expect(docked).toContain('aria-pressed="true"');
    expect(render()).toContain("parts-drawer-floating");
  });

  it("has a keyboard-operable resize separator within 240 to 480 pixels", () => {
    const markup = render();

    expect(markup).toContain('role="separator"');
    expect(markup).toContain('aria-valuemin="240"');
    expect(markup).toContain('aria-valuemax="480"');
    expect(markup).toContain('tabindex="0"');
  });

  it("is empty but usable before the catalog has loaded", () => {
    const markup = render({ catalog: null });

    expect(markup).toContain('role="listbox"');
    expect(markup).not.toContain('role="option"');
  });

  it("shows a symbol preview for every catalog part and no placeholder", () => {
    const markup = render();

    expect(markup.match(/<svg/g)).toHaveLength(catalog.componentTypes.length);
    expect(markup.match(/part-preview-placeholder/g)).toBeNull();
  });

  it("offers every part, because every part has an editor symbol", () => {
    const markup = render();

    expect(markup.match(/aria-disabled="true"/g)).toBeNull();
    expect(markup.match(/role="option"/g)).toHaveLength(
      catalog.componentTypes.length,
    );
  });
});
