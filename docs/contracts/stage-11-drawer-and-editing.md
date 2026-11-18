# Stage 11 Contract: The Parts Drawer and Double-Click Editing

**Status:** Draft for review by the Engine and Application workstreams (issue #182).
**Scope:** A sliding drawer that lists the catalog parts by category with search, favorites and recents, previews drawn from the symbols, and placement by click, drag and keyboard; the metadata the Rust catalog must carry for it; and a double-click on a part that opens an editor for its parameters, using the same metadata, validation and commands as the inspector. This document amends the earlier contracts where it says so and nothing else.

The drawer and the editor are interface features. They add no model, no equation, no result and no project field, and the interface calculates nothing in them.

## 1. Boundaries and compatibility

### 1.1 What Stage 11 adds

- A **parts drawer** (section 3) that replaces the flat row of part buttons in the editor toolbar. The tools that are not catalog parts (Select, Wire, Junction, Ground, DC rail) stay on the toolbar.
- **Catalog metadata** (section 2): a category and search keywords per part, and the ordered list of categories, owned by the Rust catalog.
- **Symbol previews** (section 4), drawn from the same symbol definitions as the canvas.
- A **double-click editor** (section 5) for the parameters of one part, and an Enter-key way to open it.

### 1.2 Earlier text this contract amends

| Earlier text | Amendment |
|---|---|
| stage-2-editor.md, editor behavior: "The palette is populated from the Rust component catalog. Selecting a type shows a snapped placement preview; clicking places the component using catalog defaults. The placement tool stays active for repeated placement." | The part list moves into the drawer. Choosing a part still selects the placement tool; the preview, the click-to-place with catalog defaults, and the repeated placement are unchanged (section 3.5). |
| stage-1.md: the catalog entry and catalog types (`ComponentCatalogEntry`, `ComponentCatalog`) | `ComponentCatalogEntry` gains `category` and `searchKeywordKeys`; `ComponentCatalog` gains `categories` (section 2). Existing fields keep their meaning. |
| stage-6-control-circuits.md section 6.3: a press on a switch lever or a push-button actuator operates it while a run is on screen | Unchanged. A double-click never operates a device and never toggles it twice (section 5.3). |
| stage-5-assignments.md: allowed part types and locked parts | Unchanged in meaning: the drawer offers only the allowed types, and a locked part's editor is read-only (sections 3.8 and 5.6). |
| stage-8-supply-rails.md section 7: the DC rail tool and panel | Unchanged. A double-click on a rail only selects it and moves focus to its properties (section 5.2). |

### 1.3 Out of scope

Renaming a part (Stage 12, with the project format change it needs); new parts or new models; any change to assignments beyond respecting what they already restrict; a different layout for narrow windows or touch screens; synchronizing preferences between machines; fuzzy or typo-tolerant search; translating the interface (the language is decided later by the project owner, and nothing here fixes it); the choice between the IEC, GOST and ANSI symbol standards (section 4.3 only keeps the door open).

## 2. Catalog metadata (Engine-owned)

The interface keeps no hand-written list of parts, categories or keywords: they come from the Rust catalog, which is already the source of the part list.

### 2.1 Fields

- `CategoryId` is an Engine-owned enum, serialized in `snake_case` and generated for TypeScript like every shared type. Its values and their display order are `sources`, `passive`, `protection`, `switching`, `measurement`, `motors_and_loads`, `semiconductors`, and, when later stages add their parts, further values added by those stages' contracts (for example `three_phase`).
- `ComponentCatalogEntry.category: CategoryId` is required: every part has exactly one category.
- `ComponentCatalogEntry.searchKeywordKeys: string[]` is a (possibly empty) list of message keys. The interface resolves each key to a short text of alternative names and related words, for example a key `component.resistor.keywords` resolving to "resistance ohm load". The Engine supplies no English or any other text; the keys follow the existing pattern (`component.<type>.keywords`, the same `<type>` segment as `displayNameKey`).
- `ComponentCatalog.categories: CategoryId[]` lists the categories in display order, each exactly once, and every part's category appears in it. An empty category is not listed.

### 2.2 Assignment of the current parts

| Category | Parts |
|---|---|
| `sources` | `dc_voltage_source`, `ac_voltage_source`, `three_phase_source_star`, `three_phase_source_delta` |
| `passive` | `resistor`, `capacitor`, `inductor` |
| `protection` | `fuse` |
| `switching` | `switch_spst`, `push_button_no`, `push_button_nc`, `relay_spdt`, `contactor` |
| `measurement` | `voltmeter`, `ammeter`, `ac_voltmeter`, `ac_ammeter`, `wattmeter` |
| `motors_and_loads` | `pmdc_motor` |
| `semiconductors` | `diode`, `bjt_npn`, `bjt_pnp` |

The order of parts inside a category is the order of the catalog. The catalog's own order, and so every golden catalog file's order of parts, does not change; only the new fields are added.

### 2.3 Rules

- A new part is added to the catalog only with a category; a test fails for a part without one (section 8).
- The categories do not affect the project format, the Engine's behavior or any result.
- The catalog golden file (`tests/golden/stage-1-catalog.json`) and the generated types are regenerated in the same change.

## 3. The drawer

### 3.1 Shape and states

The drawer is a panel on the left of the editor with a header, a search field, and a list. It has two states:

- **Docked (pinned):** the drawer takes its width from the editor and the canvas shrinks. It stays open after a part is chosen.
- **Floating (unpinned, the default):** the drawer covers the left edge of the canvas. It closes when a part is chosen and when Escape is pressed with nothing else to cancel (section 3.7).

A toolbar button **Parts** toggles it open and closed and reflects the state with `aria-expanded`. The width is adjustable between 240 and 480 pixels and defaults to 300. The drawer opens when the application starts, floating.

The motion of opening and closing is an optional transition of at most 150 ms and is absent when the user's system asks for reduced motion.

### 3.2 Categories and the list

Parts are grouped under the categories of `ComponentCatalog.categories`, in that order, each with a heading whose text is the interface's message for `category.<id>`. A category with no visible part (because of the search or an assignment) is not shown. Each item shows the preview, the part's display name, and a toggle for the favorite.

With an empty search the list also shows, above the categories, a **Favorites** group and a **Recent** group when they are not empty (section 3.6). A part that appears in them also appears under its category.

### 3.3 Search

- The query is split on white space into tokens. Matching is case-insensitive, ignores diacritics, and treats `ё` as `е`, by normalizing both the query and the text to Unicode NFKC, lower case, and with combining marks removed.
- A part matches when **every token** is a substring of at least one of: its display name, its resolved keywords, its type ID, or its category's display name.
- Results are ranked by the best rank of any token, then by the catalog order: (1) the display name equals the query; (2) the display name starts with the query; (3) a word of the display name starts with a token; (4) a keyword starts with a token; (5) any other substring match. Ties keep the catalog order, so the ranking is deterministic.
- Results are shown flat under a heading with their count, without the category groups; clearing the query returns to the grouped list. There is no fuzzy or typo-tolerant matching.
- The search runs on every keystroke and needs no delay: the list is a few dozen items.
- A query with no match shows a message with the query, and the field stays focused.

### 3.4 Keyboard

- The drawer is a labelled region. The search field has a visible label.
- **Down** and **Up** in the search field move the highlight through the shown items; **Home** and **End** go to the first and last. The highlighted item is exposed with `aria-activedescendant` and the list has the role `listbox` with items of the role `option`.
- **Enter** chooses the highlighted part (section 3.5). **Shift+Enter** chooses it and places one part at the center of the visible canvas, snapped to the grid, selects the new part, and returns focus to the canvas (the keyboard way to place a part; the canvas otherwise needs the pointer).
- **Escape** in a non-empty search clears it; in an empty search it closes a floating drawer or does nothing in a docked one; it first cancels an unfinished gesture of the canvas as everywhere (section 3.7).
- The key **/** focuses the search field and opens the drawer when it is closed, but only when the focus is not in a text-entry field (the existing rule that canvas shortcuts never intercept typing). This is the one new shortcut of this contract.
- The list of the **toolbar's** tools and every existing shortcut keep their keys.

### 3.5 Choosing and placing

- **Click** or **Enter** on a part selects the placement tool for it, with rotation 0, exactly as the flat palette did; the drawer then follows its state (a floating drawer closes, a docked one stays).
- **Drag** from an item to the canvas (the mechanism, HTML5 drag and drop or pointer events, is left to #184, which must not break the canvas's pointer capture; a pointer-free way is Shift+Enter, section 3.4) places the part at the drop point, snapped, with the catalog defaults, as one placement; the placement tool is not left active after a drop. Dropping outside the canvas places nothing and changes nothing.
- A placement is the existing placement command: one history entry, catalog defaults, a new stable ID.
- Choosing a part adds it to **Recent**.

### 3.6 Favorites and recent parts

- They are **user preferences**, stored by the interface in the web view's local storage under one key, as a list of part type IDs. They are never written to a project file, an assignment file, the Engine or the history, and they are not shared between machines.
- Favorites hold at most 32 parts, in the order the user added them. Recent holds the last 8 distinct parts chosen, newest first.
- When the stored value cannot be read, is not what the interface wrote, or names a type that the catalog does not have, the unusable entries are dropped; the interface keeps working with what is left. When storage is unavailable or full, the preferences live for the session only and nothing is shown as an error.
- The drawer's width and its docked state are preferences too, stored with them.

### 3.7 Interaction with the canvas

A press on the canvas never opens or closes the drawer. A floating drawer covers part of the canvas; pointer events on the drawer do not reach the canvas and the other way round. Escape follows the existing order **when the canvas has the focus**: it cancels an unfinished gesture or route of the canvas first, then the active tool returns to Select, and only then does it close a floating drawer. Escape typed in the drawer's search field is a text-entry event and does not reach the canvas shortcuts: there it clears the search, and in an empty search it closes a floating drawer and does nothing in a docked one (section 3.4); it never changes the active tool.

### 3.8 Assignments and projects

In an assignment the drawer lists only the allowed types (the allowed catalog the interface already computes), the search runs over that list, and favorites and recent entries that are not allowed are not shown (they are kept, and shown again outside the assignment). The rail tool and the other non-part tools are unchanged by the drawer. Opening, saving or creating a project does not change the drawer, and a project never contains drawer state.

## 4. Symbol previews

### 4.1 Source

A preview is drawn by the interface from the resting drawing (`SymbolDefinition.primitives`) of the part's symbol: the polylines and circles of the symbol registry, rendered as inline SVG scaled to the preview box, using the current text color so that it follows the theme. There is **one source of truth for a symbol**: the canvas and the drawer use the same definition, so a preview can never differ from the placed part. A part without a symbol (none today) shows a neutral placeholder with its initials; this replaces the filter of today's palette, which leaves such a part out.

### 4.2 The pictures of the owner

The project owner will supply pictures that show how the elements should look. They are **design input, not a runtime asset**: each picture is kept as a reference in `docs/symbols/<type id>.svg` (or `.png`) with a note of its source, and the symbol of that part is then redrawn as primitives so that the canvas, the preview and the placement preview all change together. Redrawing a symbol never changes its pins, their positions or any model; a test checks the pins of every symbol against the catalog and the contract (the existing symbol contract tests). A part for which no picture exists keeps its current drawing.

### 4.3 Symbol standards

Which standard (IEC 60617, GOST, ANSI) the symbols follow, and whether the user can switch, is decided later by the project owner. The registry is not changed for it here. A later contract may add alternatives keyed by a standard, with a preference, without changing any model or any pin.

## 5. The double-click editor

### 5.1 What it is

The editor shows the **parameters of one component**, in the same widgets the inspector uses (the numeric field with units and range, the choice field, the optional field with its clear action), with labels, units, defaults, domains and ranges from the catalog. It is a panel anchored next to the part on the canvas, not a modal dialog: the circuit keeps running and the readings keep updating while it is open. It has a title with the part's label (for example `R1`), a close button, and the parameters. It adds no field the catalog does not list and computes nothing.

### 5.2 Opening and closing

**The double press.** The interaction layer recognizes a double-click itself from pointer events, not from the browser's `dblclick` event (which is unreliable under pointer capture): a **double press** is a second press of the same pointer, within 400 ms and 4 screen pixels of the first, on the same object, with no drag started by the first (the pointer did not move beyond the drag threshold of the interaction layer). The thresholds are fixed numbers of the interaction layer, not user settings.

**Opening by double press.** The editor opens on a double press **only when all of these hold**: the active tool is **Select**; no wire route, rail drawing or other gesture is in progress; the first press selected a **component**; and the press is not on an operating area (section 5.3). With **any other tool** (Place, Wire, Junction, Ground, DC rail) a double press does exactly what two presses did before this stage: a double press with the Place tool places two parts, and with the Wire tool it finishes a route as it does today; the editor does not open. A double press on empty canvas, on a wire, on a junction or on a ground marker does nothing new. A double press on a **rail** selects it and moves the keyboard focus to the rail properties of the panel; it opens no part editor.

**Opening by keyboard.** **Enter** opens the editor of the selected component, **only when the keyboard event's target is the canvas element itself**. For this the canvas surface is focusable (`tabindex` 0, with an accessible name), and a press on it focuses it; the interface does not otherwise add a canvas focus model. An Enter whose target is any other element is untouched: the activation of a toolbar button, the drawer's own Enter handling (section 3.4), a text field, and the **keyboard operation of a device by its control in the Devices panel** (stage-6 section 6.3, Space or Enter on the focused control), which lives in those controls and not on the canvas, are all unchanged. Enter on the canvas is the way to the editor for a switch or a push button and for every user who does not use a pointer.

**Closing.** **Escape** closes the editor and returns focus to the canvas; so does the close button, selecting another object, deleting the part, and replacing the document. **Undo** and **redo** keep the editor open and show the values the document then has. Only one editor is open at a time; opening another replaces it. The editor shows the part's current values from the document; if the part disappears (an undo of its placement, a document replacement), the editor closes.

### 5.3 Operator devices

While a run is on screen, a press on the lever of a switch or the actuator of a push button operates it (stage-6 section 6.3), and that is unchanged and happens on the **first press**. The rule for a double press on an operating area is one rule: **the second press of a double press is ignored, and the editor does not open**. So a double press on a switch lever toggles it **once**; a double press on a push-button actuator presses it once and releases it by the first press's release, and the second press and its release do nothing. Deliberate rapid double toggling with the pointer is therefore no longer possible; a user who wants it waits longer than the double-press interval, or uses the device's control in the Devices panel. The pointer release, leaving the actuator, cancel and lost capture release a held button exactly as before.

A double press on the rest of the symbol (outside the operating area), and Enter on the canvas with the device selected, open the editor. When no run is on screen, a switch or a push button is an ordinary part with no operating area, and a double press anywhere on it opens the editor.

### 5.4 Editing and the commands

- Every change goes through the **same path as the inspector**: the same validation and SI conversion in the field, the same document command that records one history entry, and the same synchronization with the Engine (a lightweight parameter update where the catalog allows it, a recompile for an initial condition or any other change). The editor has no second implementation of any of it, and a test checks that the inspector and the editor produce the same commands for the same input.
- A refused value stays in the field with its reason (the field's existing behavior) and changes nothing. A value the document accepts but the Engine refuses leaves the run as the Engine says (stage-3 section 3.4); the editor shows the Engine's diagnostic for the part.
- Enter in a field commits it, Escape in a field restores the committed value (the field's existing behavior); Tab moves between the fields; the editor is reachable and operable by keyboard only.
- Loads and consumers are ordinary parts with ordinary parameters (for example the load torque of the motor), so they are edited here with no special case, and the change reaches the running circuit as a live update.

### 5.5 What the editor may show beside the fields

A read-only line of the part's **current Engine readings** (for example its voltage and current), taken from the result layout by the same keys and the same formatting as the Devices panel. The interface derives nothing. When there is no run on screen, the readings are absent, not zero.

### 5.6 Assignments

A part that an assignment locks opens its editor **read-only**: the fields are shown with their values and disabled, with the inspector's existing text that parts locked by the assignment cannot be edited, and a line naming the part. A part type that an assignment does not allow cannot be placed, as before.

### 5.7 Accessibility

The editor is a labelled region (or a non-modal dialog) named after the part; opening it moves the focus to its first field; closing returns the focus to the canvas; it does not trap the focus; it is announced politely when it opens; errors use `role="alert"` as the fields already do. The double-click has a keyboard equivalent (section 5.2) everywhere.

## 6. Data, generated types, and compatibility

- The three new catalog fields are Engine-owned Rust types, exported for TypeScript with the existing generator (`src/generated/**` is never edited by hand). The catalog contract test (`crates/app/tests/catalog_contract.rs`) and the golden catalog are updated together.
- No project field, no schema version, no result slot, no frame or trace format and no command changes. Older projects open as before.
- The preferences of section 3.6 are the only persisted data of this stage, and they are not part of any contract with the Engine.

## 7. Defaults and the labels of the interface

The interface supplies the text of `category.<id>` and of the keyword keys (`component.<type>.keywords`, with the same `<type>` segment as the existing `displayNameKey`, for example `component.dcVoltageSource.name`), the drawer and the editor. The English wording is the interface's; the language of the interface is not decided by this contract. Every text is a message key, so a later translation changes no behavior.

## 8. Verification

- **Catalog** (#183): every part has a category and appears in `categories`; keyword keys exist as messages; the golden catalog and generated types agree.
- **Search** (#184): ranking and matching tests on the whole current catalog, including tokens, diacritics, `ё`, no match, empty query, and assignment-limited lists.
- **Drawer** (#184): states, keyboard operation, drag placement, Shift+Enter placement, favorites and recent (including unreadable or unknown stored data), Escape order, and the assignment restriction.
- **Previews** (#185): every symbol renders; a preview equals the symbol's primitives; the placeholder; the pin check of every symbol is unchanged.
- **Editor** (#186): double-click and Enter open it; an operating target is not opened and not double-toggled; locked parts are read-only; the inspector and the editor produce equal commands; live updates and refused values; focus and Escape; undo and redo with the editor open.
- **Acceptance** (#187): `docs/acceptance/stage-11-drawer-and-editing.md` with the commands, native desktop observations including keyboard-only use, the candidate's CI on three platforms, and the earlier workflows (assignments, rails, operating devices) intact.

## 9. Decisions that need the owner's approval

These are proposals of this draft; the first review should confirm them.

1. The categories and the assignment of the current parts (section 2.2), and the rule that a category is added by the stage that adds its parts.
2. The drawer is **floating and open on start**, with a pin; the width limits; the key **/** as the only new shortcut; **Shift+Enter** as the keyboard placement.
3. Search by substring with the ranking of section 3.3, without fuzzy matching; keywords as message keys resolved by the interface.
4. Preferences in the web view's local storage, not in a file of the application.
5. The editor as a non-modal panel next to the part, with the read-only readings of section 5.5.
6. The pictures of the owner kept as references and the symbols redrawn as primitives, with one source of truth, rather than images used directly.
7. The double-click on a rail only selects it and focuses its properties.
