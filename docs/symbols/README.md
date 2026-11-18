# Part pictures and symbols

The project owner supplies pictures that show how each element should look. A picture is design input, not a runtime asset: nothing loads it when the application runs. This is the documented way to add or replace the look of a part (docs/contracts/stage-11-drawer-and-editing.md section 4).

## The one source of truth

A part's drawing is its **symbol**, in `src/symbols/<name>.ts`: polylines and circles in the symbol's local grid, plus its pins. The canvas, the placement preview and the parts drawer all draw from the same `primitives`, so a change shows up everywhere at once. The drawer's preview (`src/ui/drawer/PartPreview.tsx`) scales those primitives into a small box and draws them in the current text color, so it follows the theme.

## Adding or replacing a picture

1. Save the supplied picture as `docs/symbols/<type id>.svg` (or `.png`), where `<type id>` is the part's catalog type ID in snake case, for example `docs/symbols/resistor.svg`. Add a line to `docs/symbols/SOURCES.md` with the picture's source and licence. A test fails if a picture's name is not a catalog type ID.
2. Redraw the symbol as primitives in `src/symbols/<name>.ts` so that it matches the picture.
3. Keep the **pins** (their IDs and positions) exactly as they are. The symbol contract test (`src/symbols/contract.test.ts`) checks every symbol's pins against the Rust catalog; a redraw that moves a pin fails it.
4. Run the tests. No Rust code, model, parameter or saved project changes.

A part for which no picture exists keeps its current drawing.

## Parts without a symbol

A catalog part that the editor has no symbol for yet (the Stage 9 sources and meters today) is still listed in the drawer. It shows a neutral placeholder with the initials of its name, and it cannot be placed until a symbol is added to `src/symbols/index.ts`. Adding that symbol (with its pins matching the catalog) is the only step needed to make it placeable; the drawer needs no change.

## Symbol standards

Which standard (IEC 60617, GOST, ANSI) the symbols follow is decided by the project owner later. The registry does not change for it here.
