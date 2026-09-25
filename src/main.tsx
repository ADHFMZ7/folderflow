import React from "react";
import ReactDOM from "react-dom/client";
import { createMockApi, SAMPLE_WORKFLOWS } from "./api/mock";
import { App } from "./app/App";
import "./theme/tokens.css";
import "./theme/base.css";

// Until the Rust side exists the app runs on the mock api. Two localStorage
// switches help when trying it out:
//   folderflow.mock.ollama = "missing"    pretend Ollama isn't running
//   folderflow.mock.workflows = "sample"  show sample workflows on the home page
const api = createMockApi({
  storage: localStorage,
  ollamaRunning: localStorage.getItem("folderflow.mock.ollama") !== "missing",
  workflows: localStorage.getItem("folderflow.mock.workflows") === "sample" ? SAMPLE_WORKFLOWS : [],
});

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <App api={api} />
  </React.StrictMode>,
);
