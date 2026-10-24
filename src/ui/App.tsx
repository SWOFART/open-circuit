import {
  useEffect,
  useMemo,
  useRef,
  useState,
  useSyncExternalStore,
} from "react";
import { EditorCore } from "../editor/EditorCore";
import {
  DIVIDER_OUTPUT_COMPONENT_ID,
  componentLabels,
  dividerSchematic,
} from "../store/fixtures";
import { SchematicDocumentStore } from "../store/document";
import { Inspector } from "./Inspector";
import { ResultPanel } from "./ResultPanel";
import { useSimulation } from "./useSimulation";

// True only in a development build that explicitly asked for the mock adapter.
const usingMockAdapter =
  import.meta.env.DEV && import.meta.env.VITE_ENGINE_ADAPTER === "mock";

export function App() {
  const canvasHost = useRef<HTMLDivElement>(null);
  const editorRef = useRef<EditorCore | null>(null);
  const [editorError, setEditorError] = useState<string | null>(null);
  const documentStore = useMemo(
    () => new SchematicDocumentStore(dividerSchematic),
    [],
  );
  const document = useSyncExternalStore(
    documentStore.subscribe,
    documentStore.getState,
    documentStore.getState,
  );
  const { state, controller } = useSimulation(dividerSchematic);
  const labels = useMemo(
    () => componentLabels(document.schematic),
    [document.schematic],
  );

  useEffect(() => {
    const host = canvasHost.current;

    if (!host) {
      return;
    }

    const editor = new EditorCore();
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
            <span>Voltage divider</span>
            <div className="editor-actions">
              <span className="editor-hint">
                {document.isDirty ? "Unsaved changes" : "Example"}
              </span>
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
              documentStore.setParameter(
                componentId,
                parameterId,
                value,
                state.catalog,
              );
              void controller?.setParameter(componentId, parameterId, value);
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
