// Holds the user's settings together with the model catalog they refer to. The
// backend owns the settings: every change goes through the api, and what it
// returns replaces what we had. See docs/api-contract.md.

import { createContext, useCallback, useContext, useEffect, useState, type ReactNode } from "react";
import { useApi } from "../api/api";
import {
  ApiError, type ConnectOutcome, type Credentials, type Model, type ModelKind, type Provider, type Settings,
  type SettingsChange, type SettingsNotice,
} from "../api/types";
import { SettingsProblem } from "./SettingsProblem";

type SettingsState = {
  settings: Settings;
  kinds: ModelKind[];
  providers: Provider[];
  /** Every model offered by the user's connections. */
  models: Model[];
  /** Why the settings aren't what was saved last time, until dismissed. */
  notice: SettingsNotice;
  dismissNotice(): void;
  update(change: SettingsChange): Promise<void>;
  /** Connects a provider. A refusal comes back as `ok: false`; nothing is saved then. */
  connect(providerId: string, credentials: Credentials): Promise<ConnectOutcome>;
  removeConnection(id: string): Promise<void>;
};

type Loaded = Pick<SettingsState, "settings" | "kinds" | "providers" | "models" | "notice">;

const SettingsContext = createContext<SettingsState | null>(null);

export function SettingsProvider({ children }: { children: ReactNode }) {
  const api = useApi();
  const [loaded, setLoaded] = useState<Loaded | null>(null);
  const [failure, setFailure] = useState<ApiError | null>(null);

  useEffect(() => {
    let live = true;
    (async () => {
      try {
        const [{ settings, notice }, kinds, providers] = await Promise.all([
          api.getSettings(), api.listModelKinds(), api.listProviders(),
        ]);
        const models = (await Promise.all(settings.connections.map((c) => api.listModels(c.id)))).flat();
        if (live) setLoaded({ settings, kinds, providers, models, notice });
      } catch (e) {
        if (live) setFailure(e instanceof ApiError ? e : new ApiError("io", String(e)));
      }
    })();
    return () => { live = false; };
  }, [api]);

  const setSettings = useCallback((settings: Settings, models?: (m: Model[]) => Model[]) =>
    setLoaded((cur) => cur && { ...cur, settings, models: models ? models(cur.models) : cur.models }), []);

  const update = useCallback(async (change: SettingsChange) => {
    setSettings(await api.updateSettings(change));
  }, [api, setSettings]);

  const connect = useCallback(async (providerId: string, credentials: Credentials) => {
    const outcome = await api.connect(providerId, credentials);
    if (outcome.ok) {
      const added = await api.listModels(outcome.connection.id);
      setSettings(outcome.settings, (models) => [...models, ...added]);
    }
    return outcome;
  }, [api, setSettings]);

  const removeConnection = useCallback(async (id: string) => {
    const settings = await api.removeConnection(id);
    setSettings(settings, (models) => models.filter((m) => m.connectionId !== id));
  }, [api, setSettings]);

  const dismissNotice = useCallback(() => setLoaded((cur) => cur && { ...cur, notice: null }), []);

  if (failure) return <SettingsProblem error={failure} />;
  if (!loaded) return null;
  return (
    <SettingsContext.Provider value={{ ...loaded, dismissNotice, update, connect, removeConnection }}>
      {children}
    </SettingsContext.Provider>
  );
}

export function useSettings(): SettingsState {
  const state = useContext(SettingsContext);
  if (!state) throw new Error("useSettings must be used inside <SettingsProvider>");
  return state;
}
