import { screen, waitFor, within } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";
import { renderApp } from "../../test/render";

const OLLAMA = { id: "ollama-1", providerId: "ollama" };
const connected = {
  setupComplete: true,
  connections: [OLLAMA],
  defaults: { llm: { connectionId: "ollama-1", modelId: "qwen3.5:9b" } },
};

describe("settings page", () => {
  it("says which version of Vela this is", async () => {
    window.location.hash = "#/settings";
    renderApp({ settings: connected });
    expect(await screen.findByText(/^Vela \d+\.\d+\.\d+$/)).toBeInTheDocument();
  });

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

describe("updates", () => {
  const updates = () => screen.findByRole("region", { name: "Updates" });

  it("checks when asked and says Vela is up to date", async () => {
    window.location.hash = "#/settings";
    const { user } = renderApp({ settings: connected });

    await user.click(await within(await updates()).findByRole("button", { name: "Check now" }));

    expect(await within(await updates()).findByText("Vela is up to date. Checked just now.")).toBeInTheDocument();
  });

  it("downloads a newer version, shows its notes, and restarts to install it", async () => {
    window.location.hash = "#/settings";
    const { user, api } = renderApp({ settings: connected, update: { version: "0.9.1", notes: "Faster OCR." } });

    await user.click(await within(await updates()).findByRole("button", { name: "Check now" }));

    const section = await updates();
    expect(await within(section).findByText("Vela 0.9.1 is ready to install.")).toBeInTheDocument();
    expect(within(section).getByText("Faster OCR.")).toBeInTheDocument();
    expect(within(section).getByText("Or it installs the next time you quit Vela.")).toBeInTheDocument();
    await user.click(within(section).getByRole("button", { name: "Restart to update" }));
    expect(await api.getUpdateStatus()).toEqual({ state: "installing", version: "0.9.1" });
    expect(await within(section).findByText("Installing Vela 0.9.1…")).toBeInTheDocument();
  });

  it("can stop checking on its own", async () => {
    window.location.hash = "#/settings";
    const { user, api } = renderApp({ settings: connected });

    await user.click(await within(await updates()).findByRole("switch", { name: "Check for updates automatically" }));

    await waitFor(async () => expect((await api.getSettings()).settings.checkForUpdates).toBe(false));
  });

  it("says a development build doesn't update itself", async () => {
    window.location.hash = "#/settings";
    renderApp({ settings: connected, update: "unavailable" });

    expect(await within(await updates()).findByText(/development build doesn't update itself/)).toBeInTheDocument();
    expect(screen.queryByRole("switch", { name: "Check for updates automatically" })).not.toBeInTheDocument();
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
    // Applied by an effect just after the first render.
    await waitFor(() => expect(theme()).toBe("dark"));
  });
});
