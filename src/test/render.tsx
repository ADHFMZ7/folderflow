import { render } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { StrictMode } from "react";
import { createMockApi, type MockOptions } from "../api/mock";
import { App } from "../app/App";

/** Renders the whole app on a mock api with no delays, in Strict Mode like main.tsx,
    so effects run twice here just as they do in development. */
export function renderApp(options: MockOptions = {}) {
  const api = createMockApi({ delayMs: 0, ...options });
  return { api, user: userEvent.setup(), ...render(<StrictMode><App api={api} /></StrictMode>) };
}
