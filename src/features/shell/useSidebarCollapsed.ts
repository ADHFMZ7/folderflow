import { useCallback, useState } from "react";

const KEY = "folderflow.sidebar";

// A preference of this window, not an app setting, so it lives in local storage.
// Storage can be unavailable; then the sidebar simply starts open.
function read(): boolean {
  try { return localStorage.getItem(KEY) === "collapsed"; } catch { return false; }
}

function write(collapsed: boolean) {
  try { localStorage.setItem(KEY, collapsed ? "collapsed" : "open"); } catch { /* not remembered */ }
}

/** Whether the sidebar is collapsed to a rail, remembered between launches. */
export function useSidebarCollapsed() {
  const [collapsed, setCollapsed] = useState(read);
  const toggle = useCallback(() => setCollapsed((c) => { write(!c); return !c; }), []);
  return { collapsed, toggle };
}
