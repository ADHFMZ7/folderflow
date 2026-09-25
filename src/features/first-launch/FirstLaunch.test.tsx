import { screen, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { renderApp } from "../../test/render";

async function toModelStep(user: ReturnType<typeof renderApp>["user"]) {
  await user.click(await screen.findByRole("button", { name: "Get started" }));
  expect(screen.getByRole("heading", { name: "Connect a model" })).toBeInTheDocument();
}

describe("first launch", () => {
  it("can be skipped, and then the home warns that no models are set up", async () => {
    const { user, api } = renderApp();
    expect(await screen.findByRole("heading", { name: "Welcome to FolderFlow" })).toBeInTheDocument();
    await toModelStep(user);

    expect(screen.getByRole("button", { name: "Continue" })).toBeDisabled();
    await user.click(screen.getByRole("button", { name: "Skip for now" }));
    await user.click(screen.getByRole("button", { name: "Finish" }));

    expect(await screen.findByRole("heading", { name: "Workflows" })).toBeInTheDocument();
    expect(screen.getByRole("status")).toHaveTextContent("No LLM and System 1 model is set up");
    expect((await api.getSettings()).settings.setupComplete).toBe(true);
  });

  it("connects a local provider and makes its model the LLM default", async () => {
    const { user } = renderApp();
    await toModelStep(user);

    await user.click(screen.getByRole("button", { name: /^Ollama/ }));
    await user.click(await screen.findByRole("button", { name: "Use Ollama" }));

    expect(await screen.findByText("Connected")).toBeInTheDocument();
    expect(screen.getByRole("combobox", { name: /LLM/ })).toHaveDisplayValue("qwen3.5:9b · Ollama");
    await user.click(screen.getByRole("button", { name: "Continue" }));
    await user.click(screen.getByRole("button", { name: "Finish" }));

    const readiness = await screen.findByRole("link", { name: "Models" });
    expect(within(readiness).getByText("qwen3.5:9b")).toBeInTheDocument();
    expect(within(readiness).getByText("Not set")).toBeInTheDocument();
    expect(screen.getByRole("status")).toHaveTextContent("No System 1 model is set up, so Classify steps won't run.");
  });

  it("says when a local provider isn't running", async () => {
    const { user } = renderApp({ ollamaRunning: false });
    await toModelStep(user);
    await user.click(screen.getByRole("button", { name: /^Ollama/ }));

    expect(await screen.findByText("Ollama isn't running on this Mac.")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Check again" })).toBeInTheDocument();
  });

  it("shows why an API key was rejected", async () => {
    const { user } = renderApp();
    await toModelStep(user);
    await user.click(screen.getByRole("button", { name: /^Anthropic/ }));
    await user.type(screen.getByLabelText("API key"), "short");
    await user.click(screen.getByRole("button", { name: "Connect" }));

    expect(await screen.findByText("That key was rejected. Check it and try again.")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Continue" })).toBeDisabled();
  });

  it("links each cloud provider to the page where its API key is made", async () => {
    const { user } = renderApp();
    await toModelStep(user);
    for (const [name, url] of [
      ["Anthropic", "https://platform.claude.com/settings/keys"],
      ["OpenAI", "https://platform.openai.com/api-keys"],
      ["Groq", "https://console.groq.com/keys"],
    ]) {
      await user.click(screen.getByRole("button", { name: new RegExp(`^${name}`) }));
      expect(screen.getByRole("link", { name: `Get an API key from ${name}` })).toHaveAttribute("href", url);
    }
  });

  it("shows a provider failure and lets the user try again", async () => {
    const { user } = renderApp({ faultyProviders: ["anthropic"] });
    await toModelStep(user);
    await user.click(screen.getByRole("button", { name: /^Anthropic/ }));
    await user.type(screen.getByLabelText("API key"), "sk-live-1234567890abcdef");
    await user.click(screen.getByRole("button", { name: "Connect" }));

    expect(await screen.findByText("Anthropic sent an answer FolderFlow couldn't read. Try again later.")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Connect" })).toBeEnabled();
  });

  it("shows a detection failure with a way to check again", async () => {
    const { user } = renderApp({ faultyProviders: ["ollama"] });
    await toModelStep(user);
    await user.click(screen.getByRole("button", { name: /^Ollama/ }));

    expect(await screen.findByText("Ollama sent an answer FolderFlow couldn't read. Try again later.")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Check again" })).toBeInTheDocument();
  });

  it("keeps no trace of a key after connecting", async () => {
    const KEY = "sk-live-1234567890abcdef";
    const { user } = renderApp();
    await toModelStep(user);
    await user.click(screen.getByRole("button", { name: /^Anthropic/ }));
    await user.type(screen.getByLabelText("API key"), KEY);
    await user.click(screen.getByRole("button", { name: "Connect" }));

    expect(await screen.findByText("Connected")).toBeInTheDocument();
    expect(screen.queryByLabelText("API key")).not.toBeInTheDocument();
    expect(document.body.innerHTML).not.toContain(KEY);
  });

  it("fills the System 1 default when a provider offering it is connected", async () => {
    const { user } = renderApp();
    await toModelStep(user);
    await user.click(screen.getByRole("button", { name: /^Jev/ }));
    await user.type(screen.getByLabelText("API key"), "jev-key-123456");
    await user.click(screen.getByRole("button", { name: "Connect" }));

    expect(await screen.findByRole("combobox", { name: /System 1/ })).toHaveDisplayValue("Jev · Jev");
    expect(screen.getByRole("combobox", { name: /LLM/ })).toHaveDisplayValue("No connected provider offers this yet");
  });
});
