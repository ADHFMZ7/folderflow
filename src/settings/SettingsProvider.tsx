// Holds the user's settings together with the model catalog they refer to. The
// backend owns the settings: every change goes through the api, and what it
// returns replaces what we had. See docs/api-contract.md.

import { createContext, useCallback, useContext, useEffect, useState, type ReactNode } from "react";
import { useApi, type Api } from "../api/api";
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
  /** Connections whose models couldn't be loaded, with the reason. */
  modelProblems: ModelProblem[];
  retryModels(): Promise<void>;
  dismissNotice(): void;
  update(change: SettingsChange): Promise<void>;
  /** Connects a provider. A refusal comes back as `ok: false`; nothing is saved then. */
  connect(providerId: string, credentials: Credentials): Promise<ConnectOutcome>;
  removeConnection(id: string): Promise<void>;
};

export type ModelProblem = { connectionId: string; providerName: string; message: string };

type Loaded = Pick<SettingsState, "settings" | "kinds" | "providers" | "models" | "notice" | "modelProblems">;

/** Loads every connection's models. One unreachable provider doesn't stop the others, or the app. */
async function loadModels(api: Api, settings: Settings, providers: Provider[]) {
  const results = await Promise.allSettled(settings.connections.map((c) => api.listModels(c.id)));
  const models: Model[] = [];
  const modelProblems: ModelProblem[] = [];
  results.forEach((r, i) => {
    const c = settings.connections[i];
    if (r.status === "fulfilled") models.push(...r.value);
    else modelProblems.push({
      connectionId: c.id,
      providerName: providers.find((p) => p.id === c.providerId)?.name ?? c.providerId,
      message: r.reason instanceof Error ? r.reason.message : String(r.reason),
    });
  });
  return { models, modelProblems };
}

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
        const { models, modelProblems } = await loadModels(api, settings, providers);
        if (live) setLoaded({ settings, kinds, providers, models, modelProblems, notice });
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
    setLoaded((cur) => cur && { ...cur, modelProblems: cur.modelProblems.filter((p) => p.connectionId !== id) });
  }, [api, setSettings]);

  const dismissNotice = useCallback(() => setLoaded((cur) => cur && { ...cur, notice: null }), []);

  const retryModels = useCallback(async () => {
    if (!loaded) return;
    const result = await loadModels(api, loaded.settings, loaded.providers);
    setLoaded((cur) => cur && { ...cur, ...result });
  }, [api, loaded]);

  if (failure) return <SettingsProblem error={failure} />;
  if (!loaded) return null;
  return (
    <SettingsContext.Provider value={{ ...loaded, dismissNotice, update, connect, removeConnection, retryModels }}>
      {children}
    </SettingsContext.Provider>
  );
}

export function useSettings(): SettingsState {
  const state = useContext(SettingsContext);
  if (!state) throw new Error("useSettings must be used inside <SettingsProvider>");
  return state;
}
