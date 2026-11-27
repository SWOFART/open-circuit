import React from "react";
import { createRoot } from "react-dom/client";
import { applyInitialTheme } from "./theme/apply";
import { App } from "./ui/App";
import "./styles.css";

const rootElement = document.getElementById("root");

if (!rootElement) {
  throw new Error("The application root element is missing.");
}

// The theme is on the page before anything is drawn, so it never shows in the other one first.
applyInitialTheme();

createRoot(rootElement).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
