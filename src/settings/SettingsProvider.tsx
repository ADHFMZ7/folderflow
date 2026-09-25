// Holds the user's settings together with the model catalog they refer to, and
// is the one place that writes settings back through the api.

import { createContext, useCallback, useContext, useEffect, useRef, useState, type ReactNode } from "react";
import { useApi } from "../api/api";
import type { Connection, Model, ModelKind, Provider, Settings } from "../api/types";
import { fillDefaults } from "../features/models/defaults";

type SettingsState = {
  settings: Settings;
  kinds: ModelKind[];
  providers: Provider[];
  /** Every model offered by the user's connections. */
  models: Model[];
  update(change: Partial<Settings>): Promise<void>;
  /** Saves a new connection and gives any kind without a model one of its models. */
  addConnection(connection: Connection): Promise<void>;
};

const SettingsContext = createContext<SettingsState | null>(null);

export function SettingsProvider({ children }: { children: ReactNode }) {
  const api = useApi();
  const [loaded, setLoaded] = useState<Omit<SettingsState, "update" | "addConnection"> | null>(null);

  useEffect(() => {
    let live = true;
    (async () => {
      const [settings, kinds, providers] = await Promise.all([api.getSettings(), api.listModelKinds(), api.listProviders()]);
      const models = (await Promise.all(settings.connections.map((c) => api.listModels(c.id)))).flat();
      if (live) setLoaded({ settings, kinds, providers, models });
    })();
    return () => { live = false; };
  }, [api]);

  // Writers read the latest state through a ref, so two quick changes can't overwrite each other.
  const latest = useRef(loaded);
  latest.current = loaded;

  const commit = useCallback(async (next: NonNullable<typeof loaded>) => {
    latest.current = next;
    setLoaded(next);
    await api.saveSettings(next.settings);
  }, [api]);

  const update = useCallback(async (change: Partial<Settings>) => {
    const cur = latest.current;
    if (cur) await commit({ ...cur, settings: { ...cur.settings, ...change } });
  }, [commit]);

  const addConnection = useCallback(async (connection: Connection) => {
    const added = await api.listModels(connection.id);
    const cur = latest.current;
    if (!cur) return;
    const models = [...cur.models, ...added];
    await commit({
      ...cur,
      models,
      settings: {
        ...cur.settings,
        connections: [...cur.settings.connections, connection],
        defaults: fillDefaults(cur.settings.defaults, cur.kinds, models),
      },
    });
  }, [api, commit]);

  if (!loaded) return null;
  return <SettingsContext.Provider value={{ ...loaded, update, addConnection }}>{children}</SettingsContext.Provider>;
}

export function useSettings(): SettingsState {
  const state = useContext(SettingsContext);
  if (!state) throw new Error("useSettings must be used inside <SettingsProvider>");
  return state;
}
