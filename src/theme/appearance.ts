// Light or dark. The tokens follow the Mac unless <html data-theme> names one
// (see tokens.css); in the app the window gets the same theme, so the title bar
// and traffic lights match the page.

import { getCurrentWindow } from "@tauri-apps/api/window";
import { inTauri } from "../api/tauri";
import type { Appearance } from "../api/types";

export function applyAppearance(appearance: Appearance) {
  const root = document.documentElement;
  if (appearance === "system") root.removeAttribute("data-theme");
  else root.setAttribute("data-theme", appearance);
  if (inTauri()) getCurrentWindow().setTheme(appearance === "system" ? null : appearance).catch(() => {});
}
