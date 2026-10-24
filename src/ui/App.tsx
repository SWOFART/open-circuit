import { useEffect, useRef, useState } from "react";
import { EditorCore } from "../editor/EditorCore";

export function App() {
  const canvasHost = useRef<HTMLDivElement>(null);
  const [editorError, setEditorError] = useState<string | null>(null);

  useEffect(() => {
    const host = canvasHost.current;

    if (!host) {
      return;
    }

    const editor = new EditorCore();
    void editor.mount(host).catch((error: unknown) => {
      const message = error instanceof Error ? error.message : String(error);
      setEditorError(message);
    });

    return () => editor.destroy();
  }, []);

  return (
    <main className="workspace">
      <header className="app-header">
        <div>
          <p className="eyebrow">OPEN CIRCUIT</p>
          <h1>Circuit workspace</h1>
        </div>
        <p className="shell-status">Desktop shell ready</p>
      </header>

      <section className="editor-panel" aria-label="Circuit editor">
        <div className="editor-toolbar">
          <span>Untitled circuit</span>
          <span className="editor-hint">Editor scaffold</span>
        </div>
        <div ref={canvasHost} className="editor-canvas" />
        {editorError && (
          <p className="editor-error" role="alert">
            The canvas could not be initialized: {editorError}
          </p>
        )}
      </section>

      <footer className="app-footer">
        <span>Simulation engine is not connected yet.</span>
      </footer>
    </main>
  );
}
