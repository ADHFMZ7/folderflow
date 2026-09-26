import { screen, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { renderApp } from "../../test/render";

describe("main window", () => {
  it("opens on the Workflows home once setup is complete", async () => {
    renderApp({ settings: { setupComplete: true } });
    expect(await screen.findByRole("heading", { name: "Workflows" })).toBeInTheDocument();
    expect(await screen.findByRole("region", { name: "No workflows yet" })).toBeInTheDocument();
  });

  it("counts the workflows that need you", async () => {
    renderApp({ settings: { setupComplete: true }, sampleWorkflows: true });
    expect(await screen.findByRole("link", { name: /Receipts and invoices/ })).toBeInTheDocument();
    const nav = screen.getByRole("navigation", { name: "Main" });
    expect(within(nav).getByRole("link", { name: /Workflows/ })).toHaveTextContent("1");
  });

  it("still opens when a connected provider can't be reached", async () => {
    renderApp({
      settings: {
        setupComplete: true,
        connections: [{ id: "ollama-1", providerId: "ollama" }],
        defaults: { llm: { connectionId: "ollama-1", modelId: "qwen3.5:9b" } },
      },
      faultyProviders: ["ollama"],
    });

    expect(await screen.findByRole("heading", { name: "Workflows" })).toBeInTheDocument();
    expect(await screen.findByText(/Couldn't load models from Ollama/)).toBeInTheDocument();
  });

  it("moves between pages from the sidebar", async () => {
    const { user } = renderApp({ settings: { setupComplete: true } });
    await user.click(await screen.findByRole("link", { name: "Settings" }));
    expect(await screen.findByRole("heading", { name: "Models" })).toBeInTheDocument();
    expect(screen.getByRole("link", { name: "Settings" })).toHaveAttribute("aria-current", "page");
  });
});
