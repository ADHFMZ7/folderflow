import React from "react";
import ReactDOM from "react-dom/client";
import { createMockApi } from "./api/mock";
import { createTauriApi, inTauri } from "./api/tauri";
import { App } from "./app/App";
import "./theme/tokens.css";
import "./theme/base.css";

// Inside the app, the Rust core. In a plain browser (npm run dev), the mock,
// with localStorage switches for trying things out:
//   vela.mock.ollama = "missing"            pretend Ollama isn't running
//   vela.mock.workflows = "sample"          show sample workflows on the home page
//   vela.mock.settingsFile = "damaged"      start as if settings were recovered
//   vela.mock.settingsFile = "tooNew"       start as if settings came from a newer version
const api = inTauri()
  ? createTauriApi()
  : createMockApi({
    storage: localStorage,
    ollamaRunning: localStorage.getItem("vela.mock.ollama") !== "missing",
    sampleWorkflows: localStorage.getItem("vela.mock.workflows") === "sample",
    storedFile: (["damaged", "tooNew"] as const).find((v) => v === localStorage.getItem("vela.mock.settingsFile")),
  });

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <App api={api} />
  </React.StrictMode>,
);
