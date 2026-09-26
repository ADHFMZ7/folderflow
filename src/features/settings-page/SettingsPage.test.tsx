import { screen, within } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";
import { renderApp } from "../../test/render";

const OLLAMA = { id: "ollama-1", providerId: "ollama" };
const connected = {
  setupComplete: true,
  connections: [OLLAMA],
  defaults: { llm: { connectionId: "ollama-1", modelId: "qwen3.5:9b" } },
};

describe("settings page", () => {
  it("disconnects a provider after confirming, and its model stops being used", async () => {
    window.location.hash = "#/settings";
    const { user, api } = renderApp({ settings: connected });

    await user.click(await screen.findByRole("button", { name: /^Ollama/ }));
    await user.click(screen.getByRole("button", { name: "Disconnect" }));
    await user.click(screen.getByRole("button", { name: "Disconnect Ollama" }));

    expect(await screen.findByText("Ollama isn't connected.")).toBeInTheDocument();
    const readiness = screen.getByRole("link", { name: "Models" });
    expect(within(readiness).getAllByText("Not set")).toHaveLength(2);
    expect((await api.getSettings()).settings.connections).toEqual([]);
  });

  it("can back out of disconnecting", async () => {
    window.location.hash = "#/settings";
    const { user, api } = renderApp({ settings: connected });

    await user.click(await screen.findByRole("button", { name: /^Ollama/ }));
    await user.click(screen.getByRole("button", { name: "Disconnect" }));
    await user.click(screen.getByRole("button", { name: "Cancel" }));

    expect((await api.getSettings()).settings.connections).toEqual([OLLAMA]);
  });

});

describe("appearance", () => {
  const theme = () => document.documentElement.getAttribute("data-theme");
  afterEach(() => document.documentElement.removeAttribute("data-theme"));

  it("follows the Mac until the user picks light or dark", async () => {
    window.location.hash = "#/settings";
    const { user, api } = renderApp({ settings: { setupComplete: true } });

    const appearance = await screen.findByRole("radiogroup", { name: "Appearance" });
    expect(within(appearance).getByRole("radio", { name: "Match system" })).toBeChecked();
    expect(theme()).toBeNull();

    await user.click(within(appearance).getByRole("radio", { name: "Light" }));
    expect(theme()).toBe("light");
    expect((await api.getSettings()).settings.appearance).toBe("light");

    await user.click(within(appearance).getByRole("radio", { name: "Match system" }));
    expect(theme()).toBeNull();
    expect((await api.getSettings()).settings.appearance).toBe("system");
  });

  it("opens in the look chosen last time", async () => {
    renderApp({ settings: { setupComplete: true, appearance: "dark" } });

    await screen.findByRole("link", { name: "Settings" });
    expect(theme()).toBe("dark");
  });
});
