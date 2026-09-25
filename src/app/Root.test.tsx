import { screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { renderApp } from "../test/render";

describe("loading settings", () => {
  it("tells the user when a damaged settings file was set aside, until dismissed", async () => {
    const { user } = renderApp({ storedFile: "damaged" });

    const notice = await screen.findByRole("alert");
    expect(notice).toHaveTextContent("Your settings file was damaged");
    expect(notice).toHaveTextContent("settings.damaged");
    await user.click(screen.getByRole("button", { name: "Dismiss" }));

    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
    expect(screen.getByRole("heading", { name: "Welcome to FolderFlow" })).toBeInTheDocument();
  });

  it("stops at a blocking screen when settings are from a newer version", async () => {
    renderApp({ storedFile: "tooNew" });

    expect(await screen.findByRole("heading", { name: "These settings need a newer FolderFlow" })).toBeInTheDocument();
    expect(screen.queryByRole("heading", { name: "Welcome to FolderFlow" })).not.toBeInTheDocument();
    expect(screen.queryByRole("navigation", { name: "Main" })).not.toBeInTheDocument();
  });
});
