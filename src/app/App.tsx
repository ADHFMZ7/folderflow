import type { Api } from "../api/api";
import { ApiContext } from "../api/api";
import { SettingsProvider } from "../settings/SettingsProvider";
import { Root } from "./Root";

export function App({ api }: { api: Api }) {
  return (
    <ApiContext.Provider value={api}>
      <SettingsProvider>
        <Root />
      </SettingsProvider>
    </ApiContext.Provider>
  );
}
