// Pause all and the count of runs in progress, shared by the title bar and
// the Workflows page's paused banner.

import { createContext, useCallback, useContext, useEffect, useState, type ReactNode } from "react";
import { useApi } from "../../api/api";
import type { Activity } from "../../api/types";
import { useRunChanged } from "../runs/useRunChanged";

type ActivityState = { activity: Activity; pauseAll: (paused: boolean) => Promise<void> };

const ActivityContext = createContext<ActivityState | null>(null);

export function ActivityProvider({ children }: { children: ReactNode }) {
  const api = useApi();
  const [activity, setActivity] = useState<Activity>({ paused: false, running: 0 });
  const refresh = useCallback(async () => setActivity(await api.getActivity()), [api]);
  useEffect(() => { void refresh(); }, [refresh]);
  useRunChanged(() => { void refresh(); });
  const pauseAll = useCallback(async (paused: boolean) => setActivity(await api.pauseAll(paused)), [api]);
  return <ActivityContext.Provider value={{ activity, pauseAll }}>{children}</ActivityContext.Provider>;
}

export function useActivity(): ActivityState {
  const state = useContext(ActivityContext);
  if (!state) throw new Error("useActivity must be used inside <ActivityProvider>");
  return state;
}
