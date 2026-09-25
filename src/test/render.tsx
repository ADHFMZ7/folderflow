import { render } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { createMockApi, FRESH_SETTINGS, type MockOptions } from "../api/mock";
import type { Settings } from "../api/types";
import { App } from "../app/App";

/** Renders the whole app on a mock api with no delays. */
export function renderApp(options: MockOptions & { settings?: Partial<Settings> } = {}) {
  const { settings, ...mockOptions } = options;
  const api = createMockApi({ delayMs: 0, ...mockOptions });
  if (settings) api.saveSettings({ ...FRESH_SETTINGS, ...settings });
  return { api, user: userEvent.setup(), ...render(<App api={api} />) };
}
