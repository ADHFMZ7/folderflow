import { afterEach, describe, expect, it, vi } from "vitest";

const setTheme = vi.fn(async () => {});
vi.mock("@tauri-apps/api/window", () => ({ getCurrentWindow: () => ({ setTheme }) }));
const { applyAppearance } = await import("./appearance");

describe("applyAppearance", () => {
  afterEach(() => {
    delete (window as unknown as Record<string, unknown>).__TAURI_INTERNALS__;
    document.documentElement.removeAttribute("data-theme");
    setTheme.mockClear();
  });

  it("sets the page's theme, or leaves it to the system", () => {
    applyAppearance("dark");
    expect(document.documentElement.getAttribute("data-theme")).toBe("dark");
    applyAppearance("system");
    expect(document.documentElement.hasAttribute("data-theme")).toBe(false);
    expect(setTheme).not.toHaveBeenCalled();
  });

  it("in the app, gives the window the same theme so the title bar matches", () => {
    (window as unknown as Record<string, unknown>).__TAURI_INTERNALS__ = {};
    applyAppearance("light");
    expect(setTheme).toHaveBeenLastCalledWith("light");
    applyAppearance("system");
    expect(setTheme).toHaveBeenLastCalledWith(null);
  });
});
