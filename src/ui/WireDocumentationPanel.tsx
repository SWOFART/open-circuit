import { isTauri } from "@tauri-apps/api/core";
import { useEffect, useState } from "react";
import { buildGeometryEnvelope } from "../editor/geometry";
import type { Diagnostic } from "../generated/Diagnostic";
import type { Schematic } from "../generated/Schematic";
import type { StableId } from "../generated/StableId";
import type { SchematicDocumentStore } from "../store/document";
import { tauriWireDocumentation } from "../store/engineAdapter";
import { symbols } from "../symbols";
import { message } from "./messages";

interface Props {
  readonly schematic: Schematic;
  readonly store: SchematicDocumentStore;
  readonly wireId: StableId | null;
  readonly terminalId: StableId | null;
  readonly disabled: boolean;
}

export function WireDocumentationPanel({
  schematic,
  store,
  wireId,
  terminalId,
  disabled,
}: Props) {
  const [warnings, setWarnings] = useState<Diagnostic[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [pending, setPending] = useState(false);
  const native = isTauri();
  const terminal = schematic.documentation.terminals.find(
    (item) => item.id === terminalId,
  );
  const saved =
    schematic.documentation.wireNumbers.find((entry) => entry.wireId === wireId)
      ?.number ?? "";
  useEffect(() => {
    let current = true;
    setWarnings([]);
    if (native && schematic.documentation.wireNumbers.length > 0) {
      void Promise.resolve()
        .then(() =>
          tauriWireDocumentation.diagnostics(
            schematic,
            buildGeometryEnvelope(schematic, symbols),
          ),
        )
        .then((result) => {
          if (current && "Ok" in result) setWarnings(result.Ok);
        })
        .catch((failure: unknown) => {
          if (current) setError(String(failure));
        });
    }
    return () => {
      current = false;
    };
  }, [native, schematic]);

  async function numberWires() {
    setPending(true);
    setError(null);
    try {
      const result = await tauriWireDocumentation.number(
        schematic,
        buildGeometryEnvelope(schematic, symbols),
      );
      if ("Err" in result)
        throw new Error(
          result.Err.map((diagnostic) =>
            message(diagnostic.localizationKey),
          ).join(" "),
        );
      if (store.getState().schematic !== schematic) {
        throw new Error("The drawing changed. Run numbering again.");
      }
      store.addWireNumbers(result.Ok);
    } catch (failure) {
      setError(failure instanceof Error ? failure.message : String(failure));
    } finally {
      setPending(false);
    }
  }

  return (
    <section
      className="side-panel"
      aria-labelledby="wire-documentation-heading"
    >
      <h2 id="wire-documentation-heading">Terminals and wire numbers</h2>
      {terminal && (
        <form
          key={`${terminal.id}:${terminal.strip}:${terminal.label}`}
          onSubmit={(event) => {
            event.preventDefault();
            const data = new FormData(event.currentTarget);
            const strip = data.get("strip");
            const label = data.get("label");
            if (typeof strip !== "string" || typeof label !== "string") return;
            try {
              store.setTerminalName(terminal.id, strip, label);
              setError(null);
            } catch (failure) {
              setError(
                failure instanceof Error ? failure.message : String(failure),
              );
            }
          }}
        >
          <label>
            Terminal strip{" "}
            <input
              name="strip"
              defaultValue={terminal.strip}
              maxLength={8}
              disabled={disabled}
            />
          </label>
          <label>
            Terminal label{" "}
            <input
              name="label"
              defaultValue={terminal.label}
              maxLength={8}
              disabled={disabled}
            />
          </label>
          <button type="submit" disabled={disabled}>
            Apply terminal
          </button>
          <button
            type="button"
            disabled={disabled}
            onClick={() => store.removeTerminal(terminal.id)}
          >
            Remove terminal
          </button>
        </form>
      )}
      {wireId && (
        <form
          key={`${wireId}:${saved}`}
          onSubmit={(event) => {
            event.preventDefault();
            const value = new FormData(event.currentTarget).get("number");
            if (typeof value !== "string") return;
            try {
              store.setWireNumber(wireId, value);
              setError(null);
            } catch (failure) {
              setError(
                failure instanceof Error ? failure.message : String(failure),
              );
            }
          }}
        >
          <label>
            Selected wire{" "}
            <input
              name="number"
              defaultValue={saved}
              maxLength={16}
              disabled={disabled || pending}
            />
          </label>
          <button type="submit" disabled={disabled || pending}>
            Apply
          </button>
        </form>
      )}
      {native && (
        <button
          type="button"
          disabled={disabled || pending || schematic.wires.length === 0}
          onClick={() => void numberWires()}
        >
          Number the wires
        </button>
      )}
      {error && <p role="alert">{error}</p>}
      {warnings.length > 0 && (
        <ul aria-label="Wire number warnings">
          {warnings.map((warning, index) => (
            <li key={`${warning.code}:${index}`}>
              {message(warning.localizationKey)}
            </li>
          ))}
        </ul>
      )}
    </section>
  );
}
