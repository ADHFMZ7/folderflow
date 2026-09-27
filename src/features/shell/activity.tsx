// Pause all and the count of runs in progress, shared by the title bar and
// the Workflows page's paused banner. Kept up to date by the core's
// activity-changed event, so a pause from the menu bar shows here too.

import { createContext, useCallback, useContext, useEffect, useState, type ReactNode } from "react";
import { useApi } from "../../api/api";
import type { Activity } from "../../api/types";

type ActivityState = { activity: Activity; pauseAll: (paused: boolean) => Promise<void> };

const ActivityContext = createContext<ActivityState | null>(null);

export function ActivityProvider({ children }: { children: ReactNode }) {
  const api = useApi();
  const [activity, setActivity] = useState<Activity>({ paused: false, running: 0, needsYou: 0 });
  useEffect(() => {
    const stop = api.onActivityChanged(setActivity);
    void api.getActivity().then(setActivity);
    return stop;
  }, [api]);
  const pauseAll = useCallback(async (paused: boolean) => setActivity(await api.pauseAll(paused)), [api]);
  return <ActivityContext.Provider value={{ activity, pauseAll }}>{children}</ActivityContext.Provider>;
}

export function useActivity(): ActivityState {
  const state = useContext(ActivityContext);
  if (!state) throw new Error("useActivity must be used inside <ActivityProvider>");
  return state;
}
