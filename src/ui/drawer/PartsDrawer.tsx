import { useEffect, useRef, useState } from "react";
import type { ComponentCatalog } from "../../generated/ComponentCatalog";
import type { ComponentCatalogEntry } from "../../generated/ComponentCatalogEntry";
import type { ComponentTypeId } from "../../generated/ComponentTypeId";
import { symbols } from "../../symbols";
import { message } from "../messages";
import {
  buildDrawerView,
  searchKeyAction,
  type DrawerGroup,
} from "./drawerModel";
import {
  clampDrawerWidth,
  DRAWER_MAX_WIDTH,
  DRAWER_MIN_WIDTH,
  toggleFavorite,
  type DrawerPreferences,
} from "./drawerPreferences";
import { categoryKey } from "./partSearch";

export const DRAWER_ID = "parts-drawer";
const LISTBOX_ID = "parts-drawer-list";
const DRAG_THRESHOLD_PX = 4;
const RESIZE_STEP_PX = 20;

export interface PartsDrawerProps {
  /** The catalog already limited by an assignment; null until it has loaded. */
  readonly catalog: ComponentCatalog | null;
  readonly preferences: DrawerPreferences;
  readonly onPreferencesChange: (next: DrawerPreferences) => void;
  readonly disabled?: boolean;
  /** Click or Enter: select the placement tool for the part. */
  readonly onChoose: (typeId: ComponentTypeId) => void;
  /** Shift+Enter: place one part at the center of the canvas. */
  readonly onPlaceAtCenter: (typeId: ComponentTypeId) => void;
  /** A pointer drag of a part ended at this client position (anywhere on screen). */
  readonly onDrop: (typeId: ComponentTypeId, x: number, y: number) => void;
  /** Escape in an empty search, or the Close button. */
  readonly onClose: () => void;
  /** Reaches the search field, for the `/` shortcut. */
  readonly searchRef?: React.Ref<HTMLInputElement>;
  /** For tests and server rendering: the search text to start with. */
  readonly initialQuery?: string;
}

const isPlaceable = (typeId: ComponentTypeId): boolean =>
  Boolean(symbols[typeId]);

function optionId(group: string, typeId: string): string {
  return `parts-option-${group}-${typeId}`;
}

function optionIdsInOrder(
  groups: readonly DrawerGroup[],
  results: readonly ComponentCatalogEntry[] | null,
): string[] {
  return results === null
    ? groups.flatMap((group) =>
        group.entries.map((entry) => optionId(group.id, entry.typeId)),
      )
    : results.map((entry) => optionId("results", entry.typeId));
}

/**
 * The sliding parts drawer (docs/contracts/stage-11-drawer-and-editing.md section 3): categories,
 * search, favorites and recent parts, and three ways to place a part: click or Enter, drag to
 * the canvas, and Shift+Enter.
 */
export function PartsDrawer({
  catalog,
  preferences,
  onPreferencesChange,
  disabled = false,
  onChoose,
  onPlaceAtCenter,
  onDrop,
  onClose,
  searchRef,
  initialQuery = "",
}: PartsDrawerProps) {
  const [query, setQuery] = useState(initialQuery);
  const [highlight, setHighlight] = useState(-1);
  const [drag, setDrag] = useState<{
    readonly name: string;
    readonly x: number;
    readonly y: number;
  } | null>(null);
  const suppressClick = useRef(false);
  const view = buildDrawerView(catalog, preferences, query, isPlaceable);
  const optionIds = optionIdsInOrder(view.groups, view.results);
  const active = highlight >= 0 && highlight < view.items.length;
  const activeId = active ? optionIds[highlight] : undefined;

  useEffect(() => {
    if (activeId) {
      document.getElementById(activeId)?.scrollIntoView?.({ block: "nearest" });
    }
  }, [activeId]);

  const changeQuery = (next: string) => {
    setQuery(next);
    setHighlight(-1);
  };

  const onKeyDown = (event: React.KeyboardEvent<HTMLInputElement>) => {
    const action = searchKeyAction({
      key: event.key,
      shiftKey: event.shiftKey,
      highlight,
      count: view.items.length,
      query,
    });

    switch (action.kind) {
      case "none":
        return;
      case "highlight":
        event.preventDefault();
        setHighlight(action.index);
        return;
      case "choose": {
        const target = view.items[action.index];
        event.preventDefault();
        if (!target || disabled) {
          return;
        }
        if (action.placeAtCenter) {
          onPlaceAtCenter(target.typeId);
        } else {
          onChoose(target.typeId);
        }
        return;
      }
      case "clear":
        event.preventDefault();
        event.stopPropagation();
        changeQuery("");
        return;
      case "close":
        event.preventDefault();
        event.stopPropagation();
        onClose();
        return;
    }
  };

  const startDrag = (
    event: React.PointerEvent<HTMLElement>,
    entry: ComponentCatalogEntry,
  ) => {
    if (disabled || event.button !== 0) {
      return;
    }

    const startX = event.clientX;
    const startY = event.clientY;
    const name = message(entry.displayNameKey);
    let dragging = false;

    const stop = () => {
      window.removeEventListener("pointermove", move);
      window.removeEventListener("pointerup", finish);
      window.removeEventListener("pointercancel", cancel);
      setDrag(null);
    };
    const move = (moved: PointerEvent) => {
      if (
        !dragging &&
        Math.hypot(moved.clientX - startX, moved.clientY - startY) <
          DRAG_THRESHOLD_PX
      ) {
        return;
      }
      dragging = true;
      setDrag({ name, x: moved.clientX, y: moved.clientY });
    };
    const finish = (ended: PointerEvent) => {
      stop();
      if (dragging) {
        // The click that follows a drag must not also choose the part.
        suppressClick.current = true;
        window.setTimeout(() => {
          suppressClick.current = false;
        }, 0);
        onDrop(entry.typeId, ended.clientX, ended.clientY);
      }
    };
    const cancel = () => stop();

    window.addEventListener("pointermove", move);
    window.addEventListener("pointerup", finish);
    window.addEventListener("pointercancel", cancel);
  };

  const renderEntry = (group: string, entry: ComponentCatalogEntry) => {
    const id = optionId(group, entry.typeId);
    const name = message(entry.displayNameKey);
    const favorite = preferences.favorites.includes(entry.typeId);

    return (
      <div className="parts-row" role="none" key={id}>
        <div
          id={id}
          role="option"
          aria-selected={id === activeId}
          aria-disabled={disabled || undefined}
          className={
            id === activeId
              ? "parts-option parts-option-active"
              : "parts-option"
          }
          onPointerDown={(event) => startDrag(event, entry)}
          onClick={() => {
            if (!suppressClick.current && !disabled) {
              onChoose(entry.typeId);
            }
          }}
        >
          {name}
        </div>
        <button
          type="button"
          className="parts-favorite"
          aria-pressed={favorite}
          aria-label={
            favorite
              ? `Remove ${name} from favorites`
              : `Add ${name} to favorites`
          }
          onClick={() =>
            onPreferencesChange(toggleFavorite(preferences, entry.typeId))
          }
        >
          {favorite ? "★" : "☆"}
        </button>
      </div>
    );
  };

  const renderGroup = (group: DrawerGroup) => {
    const headingId = `parts-heading-${group.id}`;
    const key =
      group.id === "favorites" || group.id === "recent"
        ? `category.${group.id}`
        : categoryKey(group.id);

    return (
      <div role="group" aria-labelledby={headingId} key={group.id}>
        <h3 className="parts-heading" id={headingId}>
          {message(key)}
        </h3>
        {group.entries.map((entry) => renderEntry(group.id, entry))}
      </div>
    );
  };

  const resizeTo = (width: number) =>
    onPreferencesChange({ ...preferences, width: clampDrawerWidth(width) });

  const startResize = (event: React.PointerEvent<HTMLElement>) => {
    event.preventDefault();
    const startX = event.clientX;
    const startWidth = preferences.width;
    const move = (moved: PointerEvent) =>
      resizeTo(startWidth + moved.clientX - startX);
    const end = () => {
      window.removeEventListener("pointermove", move);
      window.removeEventListener("pointerup", end);
    };
    window.addEventListener("pointermove", move);
    window.addEventListener("pointerup", end);
  };

  return (
    <aside
      id={DRAWER_ID}
      className={
        preferences.docked
          ? "parts-drawer parts-drawer-docked"
          : "parts-drawer parts-drawer-floating"
      }
      style={{ width: preferences.width }}
      aria-label="Parts"
    >
      <div className="parts-header">
        <h2>Parts</h2>
        <button
          type="button"
          className="toolbar-button"
          aria-pressed={preferences.docked}
          title="Keep the drawer open beside the canvas"
          onClick={() =>
            onPreferencesChange({
              ...preferences,
              docked: !preferences.docked,
            })
          }
        >
          Pin
        </button>
        <button type="button" className="toolbar-button" onClick={onClose}>
          Close
        </button>
      </div>
      <label className="parts-search-label" htmlFor="parts-search">
        Search parts
      </label>
      <input
        id="parts-search"
        ref={searchRef}
        className="parts-search"
        type="search"
        role="combobox"
        autoComplete="off"
        spellCheck={false}
        aria-expanded="true"
        aria-controls={LISTBOX_ID}
        aria-activedescendant={activeId}
        value={query}
        onChange={(event) => changeQuery(event.target.value)}
        onKeyDown={onKeyDown}
      />
      <div
        id={LISTBOX_ID}
        className="parts-list"
        role="listbox"
        aria-label="Parts"
      >
        {view.results === null
          ? view.groups.map(renderGroup)
          : view.results.map((entry) => renderEntry("results", entry))}
      </div>
      {view.results !== null && (
        <p className="parts-status" role="status">
          {view.results.length === 0
            ? `No part matches “${query.trim()}”.`
            : `${view.results.length} ${view.results.length === 1 ? "part" : "parts"} found.`}
        </p>
      )}
      <div
        className="parts-resize"
        role="separator"
        aria-orientation="vertical"
        aria-label="Resize the parts drawer"
        aria-valuemin={DRAWER_MIN_WIDTH}
        aria-valuemax={DRAWER_MAX_WIDTH}
        aria-valuenow={preferences.width}
        tabIndex={0}
        onPointerDown={startResize}
        onKeyDown={(event) => {
          if (event.key === "ArrowLeft") {
            event.preventDefault();
            resizeTo(preferences.width - RESIZE_STEP_PX);
          } else if (event.key === "ArrowRight") {
            event.preventDefault();
            resizeTo(preferences.width + RESIZE_STEP_PX);
          }
        }}
      />
      {drag && (
        <div
          className="parts-drag-ghost"
          aria-hidden="true"
          style={{ left: drag.x + 12, top: drag.y + 12 }}
        >
          {drag.name}
        </div>
      )}
    </aside>
  );
}
