import {
  useEffect,
  useMemo,
  useRef,
  useState,
  useSyncExternalStore,
} from "react";
import type { EditorCommand } from "../editor/commands";
import { EditorCore } from "../editor/EditorCore";
import type { EditorTool } from "../editor/interaction";
import {
  DIVIDER_OUTPUT_COMPONENT_ID,
  componentLabels,
  dividerSchematic,
} from "../store/fixtures";
import { SchematicDocumentStore } from "../store/document";
import { applyEditorCommand } from "./editorCommands";
import { Inspector } from "./Inspector";
import { Palette } from "./Palette";
import { ResultPanel } from "./ResultPanel";
import { useSimulation } from "./useSimulation";

// True only in a development build that explicitly asked for the mock adapter.
const usingMockAdapter =
  import.meta.env.DEV && import.meta.env.VITE_ENGINE_ADAPTER === "mock";

export function App() {
  const canvasHost = useRef<HTMLDivElement>(null);
  const editorRef = useRef<EditorCore | null>(null);
  const [editorError, setEditorError] = useState<string | null>(null);
  const [editError, setEditError] = useState<string | null>(null);
  const [tool, setTool] = useState<EditorTool>({ kind: "select" });
  const documentStore = useMemo(
    () => new SchematicDocumentStore(dividerSchematic),
    [],
  );
  const document = useSyncExternalStore(
    documentStore.subscribe,
    documentStore.getState,
    documentStore.getState,
  );
  // The store's own copy, so the first document sync below is a no-op.
  const initialSchematic = useMemo(
    () => documentStore.getState().schematic,
    [documentStore],
  );
  const { state, controller } = useSimulation(initialSchematic);
  const labels = useMemo(
    () => componentLabels(document.schematic),
    [document.schematic],
  );

  // The editor is created once; commands always use the latest catalog.
  const commandHandler = useRef<(command: EditorCommand) => void>(() => {});
  useEffect(() => {
    commandHandler.current = (command) => {
      try {
        applyEditorCommand(documentStore, command, state.catalog);
        setEditError(null);
      } catch (error) {
        setEditError(error instanceof Error ? error.message : String(error));
      }
    };
  });

  useEffect(() => {
    const host = canvasHost.current;

    if (!host) {
      return;
    }

    const editor = new EditorCore({
      onCommand: (command) => commandHandler.current(command),
      onToolChange: setTool,
    });
    editorRef.current = editor;
    void editor.mount(host).catch((error: unknown) => {
      const message = error instanceof Error ? error.message : String(error);
      setEditorError(message);
    });

    return () => {
      editor.destroy();
      editorRef.current = null;
    };
  }, []);

  useEffect(() => {
    editorRef.current?.setSchematic(document.schematic, labels);
  }, [document.schematic, labels]);

  // Every document change reaches the Engine; parameter edits are sent
  // directly by the Inspector, so this then finds nothing new.
  useEffect(() => {
    void controller?.syncDocument(document.schematic);
  }, [controller, document.schematic]);

  return (
    <main className="workspace">
      <header className="app-header">
        <div>
          <p className="eyebrow">OPEN CIRCUIT</p>
          <h1>Circuit workspace</h1>
        </div>
        <p className="shell-status">Desktop shell ready</p>
      </header>

      <div className="workspace-body">
        <section className="editor-panel" aria-label="Circuit editor">
          <div className="editor-toolbar">
            <Palette
              catalog={state.catalog}
              tool={tool}
              onSelectTool={(next) => editorRef.current?.setTool(next)}
            />
            <div className="editor-actions">
              <span className="editor-hint">
                {document.isDirty ? "Unsaved changes" : "Example"}
              </span>
              <button
                type="button"
                className="toolbar-button"
                disabled={!document.canUndo}
                onClick={() => commandHandler.current({ kind: "undo" })}
              >
                Undo
              </button>
              <button
                type="button"
                className="toolbar-button"
                disabled={!document.canRedo}
                onClick={() => commandHandler.current({ kind: "redo" })}
              >
                Redo
              </button>
              <button
                type="button"
                className="toolbar-button"
                onClick={() => editorRef.current?.fitToCircuit()}
              >
                Fit to circuit
              </button>
            </div>
          </div>
          <div ref={canvasHost} className="editor-canvas" />
          {editError && (
            <p className="editor-error" role="alert">
              The edit was not applied: {editError}
            </p>
          )}
          {editorError && (
            <p className="editor-error" role="alert">
              The canvas could not be initialized: {editorError}
            </p>
          )}
        </section>

        <aside className="side">
          <Inspector
            state={state}
            schematic={document.schematic}
            labels={labels}
            onCommit={(componentId, parameterId, value) => {
              if (!state.catalog) return;
              try {
                documentStore.setParameter(
                  componentId,
                  parameterId,
                  value,
                  state.catalog,
                );
                setEditError(null);
              } catch (error) {
                setEditError(
                  error instanceof Error ? error.message : String(error),
                );
                return;
              }
              void controller?.syncDocument(
                documentStore.getState().schematic,
                { componentId, parameterId, value },
              );
            }}
          />
          <ResultPanel
            state={state}
            labels={labels}
            outputComponentId={DIVIDER_OUTPUT_COMPONENT_ID}
          />
        </aside>
      </div>

      <footer className="app-footer">
        {usingMockAdapter ? (
          <span role="status">
            Development mock: predetermined fixture responses, not the Rust
            engine.
          </span>
        ) : (
          <span>Values come from the Rust engine only.</span>
        )}
      </footer>
    </main>
  );
}
