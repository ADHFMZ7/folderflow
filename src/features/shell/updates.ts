// Where updates stand, kept up to date by the core's update-changed event, so
// a check from the menu bar or on the schedule shows here too. See
// docs/engine.md, "Updates".

import { useCallback, useEffect, useState } from "react";
import { useApi } from "../../api/api";
import { ApiError, type UpdateStatus } from "../../api/types";

export function useUpdates() {
  const api = useApi();
  const [status, setStatus] = useState<UpdateStatus>({ state: "idle", checkedAt: null });
  const [error, setError] = useState<string | null>(null);
  useEffect(() => {
    const stop = api.onUpdateChanged(setStatus);
    void api.getUpdateStatus().then(setStatus);
    return stop;
  }, [api]);

  const check = useCallback(async () => {
    setError(null);
    setStatus(await api.checkForUpdates());
  }, [api]);
  const restart = useCallback(async () => {
    setError(null);
    try {
      await api.restartToUpdate();
    } catch (e) {
      setError(e instanceof ApiError ? e.message : "The update couldn't be installed.");
    }
  }, [api]);
  return { status, error, check, restart };
}
