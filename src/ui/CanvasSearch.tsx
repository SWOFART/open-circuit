import { useEffect, useId, useMemo, useRef, useState } from "react";
import {
  searchDocument,
  searchKeyAction,
  type SearchResult,
} from "../editor/search";
import type { ComponentTypeId } from "../generated/ComponentTypeId";
import type { Schematic } from "../generated/Schematic";
import type { StableId } from "../generated/StableId";

export interface CanvasSearchProps {
  readonly schematic: Schematic;
  readonly labels: ReadonlyMap<StableId, string>;
  readonly typeName: (typeId: ComponentTypeId) => string;
  /** Selects the result and centers the view on it. */
  readonly onChoose: (result: SearchResult) => void;
  /** Closes the field; the selection is kept. */
  readonly onClose: () => void;
}

const KIND_NAMES: Readonly<Record<SearchResult["kind"], string>> = {
  component: "Part",
  rail: "Rail",
  note: "Note",
};

/**
 * The search field of the canvas (stage 16 contract, section 7). Enter or a click on a result
 * selects it and centers the view on it, Up and Down move through the results, and Escape closes
 * the field. It reads the document and never changes it.
 */
export function CanvasSearch({
  schematic,
  labels,
  typeName,
  onChoose,
  onClose,
}: CanvasSearchProps) {
  const listId = useId();
  const input = useRef<HTMLInputElement>(null);
  const [query, setQuery] = useState("");
  const [active, setActive] = useState(0);
  const results = useMemo(
    () => searchDocument(schematic, labels, typeName, query),
    [schematic, labels, typeName, query],
  );
  const empty = query.trim() === "";
  const current = Math.min(active, Math.max(results.length - 1, 0));

  useEffect(() => {
    input.current?.focus();
    input.current?.select();
  }, []);

  const onKeyDown = (event: React.KeyboardEvent): void => {
    const step = searchKeyAction(event.key, current, results.length);
    if (!step) {
      return;
    }

    event.preventDefault();
    if (step.action === "close") {
      onClose();
    } else if (step.action === "move") {
      setActive(step.current);
    } else {
      const result = results[current];
      if (result) onChoose(result);
    }
  };

  return (
    <div className="canvas-search" role="search">
      <input
        ref={input}
        type="search"
        role="combobox"
        aria-label="Search the drawing"
        aria-expanded={results.length > 0}
        aria-controls={listId}
        aria-autocomplete="list"
        aria-activedescendant={
          results.length > 0 ? `${listId}-${current}` : undefined
        }
        placeholder="Search parts, rails and notes"
        value={query}
        onChange={(event) => {
          setQuery(event.target.value);
          setActive(0);
        }}
        onKeyDown={onKeyDown}
      />
      <ul id={listId} role="listbox" aria-label="Search results">
        {results.map((result, index) => (
          <li
            key={`${result.kind}-${result.id}`}
            id={`${listId}-${index}`}
            role="option"
            aria-selected={index === current}
            className={index === current ? "search-result-active" : undefined}
            // The field keeps the focus, so a click is a press, not a blur first.
            onMouseDown={(event) => event.preventDefault()}
            onClick={() => onChoose(result)}
          >
            <span className="search-kind">{KIND_NAMES[result.kind]}</span>{" "}
            <span className="search-name">{result.name}</span>
            {result.typeName && (
              <span className="search-type"> ({result.typeName})</span>
            )}
          </li>
        ))}
      </ul>
      {!empty && results.length === 0 && (
        <p className="panel-note" role="status">
          Nothing found
        </p>
      )}
    </div>
  );
}
