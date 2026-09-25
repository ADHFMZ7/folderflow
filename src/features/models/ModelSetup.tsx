// Connecting providers and choosing a model per kind. Used by first launch and
// by Settings, so both always offer the same choices.

import { Fragment, useState, type ReactNode } from "react";
import type { Connection, Provider } from "../../api/types";
import { useSettings } from "../../settings/SettingsProvider";
import { Badge, Field, Select } from "../../ui";
import { ConnectProvider } from "./ConnectProvider";
import { findModel } from "./defaults";
import styles from "./models.module.css";

const GROUPS: { location: Provider["location"]; title: string }[] = [
  { location: "local", title: "On this Mac" },
  { location: "cloud", title: "Cloud" },
];

export function ModelSetup() {
  const { providers, settings, addConnection } = useSettings();
  const [openId, setOpenId] = useState<string | null>(null);

  const connected = (providerId: string) => settings.connections.some((c) => c.providerId === providerId);
  const onConnected = async (connection: Connection) => {
    await addConnection(connection);
    setOpenId(null);
  };

  return (
    <div className={styles.stack}>
      <ProviderList providers={providers} openId={openId} onToggle={(id) => setOpenId(openId === id ? null : id)}
        isConnected={connected} renderOpen={(p) => <ConnectProvider provider={p} onConnected={onConnected} />} />
      {settings.connections.length > 0 && <KindDefaults />}
    </div>
  );
}

function ProviderList({ providers, openId, onToggle, isConnected, renderOpen }: {
  providers: Provider[];
  openId: string | null;
  onToggle: (id: string) => void;
  isConnected: (id: string) => boolean;
  renderOpen: (provider: Provider) => ReactNode;
}) {
  const { kinds } = useSettings();
  const kindName = (id: string) => kinds.find((k) => k.id === id)?.name ?? id;

  return (
    <div className={styles.stack}>
      {GROUPS.map((g) => (
        <section key={g.location} className={styles.group} aria-label={g.title}>
          <h4 className={styles.groupTitle}>{g.title}</h4>
          {providers.filter((p) => p.location === g.location).map((p) => (
            <Fragment key={p.id}>
              <button type="button" className={openId === p.id ? styles.providerOpen : styles.provider}
                aria-expanded={openId === p.id} onClick={() => onToggle(p.id)}>
                <span className={styles.providerName}>{p.name}</span>
                <span className={styles.kinds}>{p.kinds.map((k) => <Badge key={k}>{kindName(k)}</Badge>)}</span>
                {isConnected(p.id) && <Badge tone="ok">Connected</Badge>}
              </button>
              {openId === p.id && renderOpen(p)}
            </Fragment>
          ))}
        </section>
      ))}
    </div>
  );
}

/** One row per model kind: which connected model it uses by default. */
export function KindDefaults() {
  const { kinds, models, providers, settings, update } = useSettings();
  const providerName = (connectionId: string) => {
    const conn = settings.connections.find((c) => c.id === connectionId);
    return providers.find((p) => p.id === conn?.providerId)?.name ?? "";
  };
  const value = (kindId: string) => {
    const m = findModel(models, settings.defaults[kindId]);
    return m ? `${m.connectionId}|${m.id}` : "";
  };

  return (
    <section className={styles.stack} aria-label="Default models">
      <h4 className={styles.groupTitle}>Used for</h4>
      {kinds.map((kind) => {
        const options = models.filter((m) => m.kind === kind.id);
        return (
          <Field key={kind.id} label={kind.name} hint={`${kind.description} Used by ${kind.usedBy.join(", ")}.`}>
            <Select value={value(kind.id)} onChange={(e) => {
              const [connectionId, modelId] = e.target.value.split("|");
              update({ defaults: { ...settings.defaults, [kind.id]: e.target.value ? { connectionId, modelId } : null } });
            }}>
              <option value="">{options.length ? "Not set" : "No connected provider offers this yet"}</option>
              {options.map((m) => (
                <option key={`${m.connectionId}|${m.id}`} value={`${m.connectionId}|${m.id}`}>
                  {m.name} · {providerName(m.connectionId)}
                </option>
              ))}
            </Select>
          </Field>
        );
      })}
    </section>
  );
}
