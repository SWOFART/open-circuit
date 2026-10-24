import { useEffect, useMemo, useRef, useState } from "react";
import { EditorCore } from "../editor/EditorCore";
import { componentLabels, dividerSchematic } from "../store/fixtures";
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
  const { state, controller } = useSimulation(dividerSchematic);
  const labels = useMemo(
    () => componentLabels(state.schematic),
    [state.schematic],
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
    editorRef.current?.setSchematic(state.schematic, labels);
  }, [state.schematic, labels]);

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
            <span className="editor-hint">Read-only view</span>
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
            labels={labels}
            onCommit={(componentId, parameterId, value) => {
              void controller?.setParameter(componentId, parameterId, value);
            }}
          />
          <ResultPanel state={state} labels={labels} />
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
