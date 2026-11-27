import { useCallback, useEffect, useRef, useState } from "react";
import { isTextEntryTarget } from "../../editor/canvasInput";
import type { EditorCore } from "../../editor/EditorCore";
import { matchesShortcut } from "../../editor/shortcuts";
import type { EditorStatus } from "../../editor/interaction";
import type { ComponentCatalog } from "../../generated/ComponentCatalog";
import type { ComponentTypeId } from "../../generated/ComponentTypeId";
import type { StableId } from "../../generated/StableId";
import {
  DEFAULT_DRAWER_PREFERENCES,
  loadDrawerPreferences,
  recordRecent,
  saveDrawerPreferences,
  type DrawerPreferences,
  type PreferenceStorage,
} from "./drawerPreferences";

export interface PartsDrawerController {
  readonly open: boolean;
  readonly toggle: () => void;
  readonly close: () => void;
  readonly preferences: DrawerPreferences;
  readonly setPreferences: (next: DrawerPreferences) => void;
  readonly searchRef: React.RefObject<HTMLInputElement | null>;
  readonly choose: (typeId: ComponentTypeId) => void;
  readonly placeAtCenter: (typeId: ComponentTypeId) => void;
  readonly drop: (typeId: ComponentTypeId, x: number, y: number) => void;
}

function browserStorage(): PreferenceStorage | null {
  try {
    return typeof window === "undefined" ? null : window.localStorage;
  } catch {
    return null;
  }
}

export interface PartsDrawerInputs {
  /** The full catalog (before an assignment limits it); null until it has loaded. */
  readonly catalog: ComponentCatalog | null;
  readonly editor: React.RefObject<EditorCore | null>;
  readonly canvasHost: React.RefObject<HTMLElement | null>;
  readonly status: EditorStatus;
  /** The IDs of the components in the document, to find the one a placement adds. */
  readonly componentIds: () => readonly StableId[];
}

/**
 * The state and behavior of the parts drawer: open or closed (open at start, floating), the
 * stored preferences, the `/` shortcut, Escape closing a floating drawer after the canvas has
 * nothing left to cancel, and the three ways to place a part.
 */
export function usePartsDrawer({
  catalog,
  editor,
  canvasHost,
  status,
  componentIds,
}: PartsDrawerInputs): PartsDrawerController {
  const [open, setOpen] = useState(true);
  const [preferences, setPreferencesState] = useState(
    DEFAULT_DRAWER_PREFERENCES,
  );
  const [focusRequest, setFocusRequest] = useState(0);
  const searchRef = useRef<HTMLInputElement | null>(null);
  const loaded = useRef(false);
  const latest = useRef({ open, preferences, status });
  latest.current = { open, preferences, status };

  const hasCatalog = catalog !== null;
  const types = catalog?.componentTypes;

  // The stored preferences are read once the catalog says which parts exist.
  useEffect(() => {
    if (loaded.current || !hasCatalog || !types) {
      return;
    }
    loaded.current = true;
    setPreferencesState(
      loadDrawerPreferences(
        browserStorage(),
        new Set(types.map((entry) => entry.typeId)),
      ),
    );
  }, [hasCatalog, types]);

  const setPreferences = useCallback((next: DrawerPreferences) => {
    setPreferencesState(next);
    if (loaded.current) {
      saveDrawerPreferences(browserStorage(), next);
    }
  }, []);

  const close = useCallback(() => setOpen(false), []);
  const toggle = useCallback(() => setOpen((value) => !value), []);

  useEffect(() => {
    if (focusRequest > 0 && open) {
      searchRef.current?.focus();
    }
  }, [focusRequest, open]);

  useEffect(() => {
    // `/` reaches the search field unless the user is typing somewhere.
    const onKeyDown = (event: KeyboardEvent) => {
      if (
        !matchesShortcut(event, "parts.drawer") ||
        isTextEntryTarget(event.target)
      ) {
        return;
      }
      event.preventDefault();
      setOpen(true);
      setFocusRequest((count) => count + 1);
    };
    // Escape closes a floating drawer only when the canvas has nothing left to cancel. This
    // runs before the canvas handler, so the status it reads is the one Escape arrives at.
    const onEscape = (event: KeyboardEvent) => {
      const {
        open: isOpen,
        preferences: current,
        status: state,
      } = latest.current;
      if (
        matchesShortcut(event, "edit.escape") &&
        isOpen &&
        !current.docked &&
        !isTextEntryTarget(event.target) &&
        state.tool.kind === "select" &&
        !state.routing &&
        !state.railStarted
      ) {
        setOpen(false);
      }
    };

    window.addEventListener("keydown", onKeyDown);
    window.addEventListener("keydown", onEscape, true);
    return () => {
      window.removeEventListener("keydown", onKeyDown);
      window.removeEventListener("keydown", onEscape, true);
    };
  }, []);

  const chosen = (typeId: ComponentTypeId) => {
    setPreferences(recordRecent(latest.current.preferences, typeId));
    if (!latest.current.preferences.docked) {
      setOpen(false);
    }
  };

  const place = (
    typeId: ComponentTypeId,
    at: { readonly x: number; readonly y: number } | undefined,
  ): boolean => {
    const core = editor.current;
    if (!core) {
      return false;
    }

    const before = new Set(componentIds());
    if (!core.placeAt(typeId, at)) {
      return false;
    }

    const added = componentIds().find((id) => !before.has(id));
    if (added !== undefined) {
      core.select({ kind: "component", id: added });
    }
    return true;
  };

  return {
    open,
    toggle,
    close,
    preferences,
    setPreferences,
    searchRef,
    choose: (typeId) => {
      editor.current?.setTool({ kind: "place", typeId, rotationDeg: 0 });
      chosen(typeId);
    },
    placeAtCenter: (typeId) => {
      if (place(typeId, undefined)) {
        chosen(typeId);
        editor.current?.focusCanvas();
      }
    },
    drop: (typeId, x, y) => {
      const element = document.elementFromPoint(x, y);
      if (
        element &&
        canvasHost.current?.contains(element) &&
        place(typeId, { x, y })
      ) {
        // A drop places one part; the placement tool is not left active.
        editor.current?.setTool({ kind: "select" });
        chosen(typeId);
      }
    },
  };
}
