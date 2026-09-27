import { useEffect, useRef } from "react";
import { useApi } from "../../api/api";
import type { RunChanged } from "../../api/types";

/** Calls `listener` whenever a run changes, for as long as the component is shown. */
export function useRunChanged(listener: (change: RunChanged) => void) {
  const api = useApi();
  const latest = useRef(listener);
  latest.current = listener;
  useEffect(() => api.onRunChanged((change) => latest.current(change)), [api]);
}
