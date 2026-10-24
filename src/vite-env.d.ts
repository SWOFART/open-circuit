/// <reference types="vite/client" />

interface ImportMetaEnv {
  /**
   * Development only: set to `mock` to run the UI against predetermined
   * fixture responses. Ignored, and removed from the bundle, in production.
   */
  readonly VITE_ENGINE_ADAPTER?: string;
}

interface ImportMeta {
  readonly env: ImportMetaEnv;
}
