// One connect form for every provider, shaped by how that provider connects.

import { useEffect, useState } from "react";
import { useApi } from "../../api/api";
import type { Credentials, Provider } from "../../api/types";
import { useSettings } from "../../settings/SettingsProvider";
import { Button, ExternalLink, Field, Spinner, TextInput } from "../../ui";
import styles from "./models.module.css";

/** `onConnected` runs once the backend has saved the connection. */
type Props = { provider: Provider; onConnected: () => void };

export function ConnectProvider({ provider, onConnected }: Props) {
  return (
    <div className={styles.connect}>
      <p className={styles.muted}>{provider.privacy}</p>
      {provider.connect === "detect" && <DetectPanel provider={provider} onConnected={onConnected} />}
      {provider.connect === "apiKey" && <CredentialsForm provider={provider} onConnected={onConnected} fields={["apiKey"]} />}
      {provider.connect === "endpoint" && <CredentialsForm provider={provider} onConnected={onConnected} fields={["endpoint", "apiKey"]} />}
    </div>
  );
}

function DetectPanel({ provider, onConnected }: Props) {
  const api = useApi();
  const { connect } = useSettings();
  const [state, setState] = useState<"checking" | "found" | "connecting" | { missing: string } | { error: string }>("checking");

  const check = async () => {
    setState("checking");
    const result = await api.detect(provider.id);
    setState(result.found ? "found" : { missing: result.reason });
  };

  const use = async () => {
    setState("connecting");
    const result = await connect(provider.id, {});
    if (result.ok) onConnected();
    else setState({ error: result.error });
  };

  useEffect(() => { check(); }, [provider.id]);

  if (state === "checking") return <Spinner label={`Looking for ${provider.name}…`} />;
  if (state === "found" || state === "connecting")
    return (
      <div className={styles.row}>
        <span className={styles.ok}>✓ {provider.name} is running</span>
        <Button onClick={use} disabled={state === "connecting"}>{state === "connecting" ? "Connecting…" : `Use ${provider.name}`}</Button>
      </div>
    );
  return (
    <div className={styles.stack}>
      <span className={styles.bad}>{"missing" in state ? state.missing : state.error}</span>
      <div className={styles.row}>
        {provider.helpUrl && <ExternalLink href={provider.helpUrl}>Get {provider.name}</ExternalLink>}
        <Button variant="secondary" onClick={check}>Check again</Button>
      </div>
    </div>
  );
}

const FIELD_LABEL: Record<keyof Credentials, string> = { apiKey: "API key", endpoint: "Server address" };

function CredentialsForm({ provider, onConnected, fields }: Props & { fields: (keyof Credentials)[] }) {
  const { connect } = useSettings();
  const [values, setValues] = useState<Credentials>({});
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const required = fields[0];

  const submit = async () => {
    setBusy(true);
    setError(null);
    const result = await connect(provider.id, values);
    setBusy(false);
    // On success the form unmounts, and the key typed into it goes with it.
    if (result.ok) onConnected();
    else setError(result.error);
  };

  return (
    <form className={styles.stack} onSubmit={(e) => { e.preventDefault(); submit(); }}>
      {fields.map((f) => (
        <Field key={f} label={f === required ? FIELD_LABEL[f] : `${FIELD_LABEL[f]} (optional)`}
          hint={f === "apiKey" && provider.keyUrl && <ExternalLink href={provider.keyUrl}>Get an API key from {provider.name}</ExternalLink>}>
          <TextInput
            type={f === "apiKey" ? "password" : "url"}
            placeholder={f === "apiKey" ? "Paste your key" : "http://localhost:8080"}
            value={values[f] ?? ""}
            onChange={(e) => { setValues({ ...values, [f]: e.target.value }); setError(null); }}
          />
        </Field>
      ))}
      <div className={styles.row}>
        <Button type="submit" disabled={busy || !values[required]}>{busy ? "Checking…" : "Connect"}</Button>
        {error && <span className={styles.bad}>{error}</span>}
      </div>
      {fields.includes("apiKey") && <p className={styles.muted}>Keys are stored in your macOS Keychain.</p>}
    </form>
  );
}
