// Settings › Updates: the automatic check, where updates stand, and Restart to
// update once one is downloaded. See docs/engine.md, "Updates".

import { useSettings } from "../../settings/SettingsProvider";
import { Button, Spinner, Toggle } from "../../ui";
import { ago } from "../runs/format";
import { useUpdates } from "../shell/updates";
import styles from "./SettingsPage.module.css";

export function Updates() {
  const { settings, update } = useSettings();
  const { status, error, check, restart } = useUpdates();

  if (status.state === "unavailable") {
    return <p className={styles.muted}>This development build doesn't update itself. Vela.app does.</p>;
  }
  return (
    <>
      <Toggle label="Check for updates automatically" checked={settings.checkForUpdates}
        onChange={(checkForUpdates) => update({ checkForUpdates })} />
      <p className={styles.muted}>Vela asks GitHub for its newest version. Nothing about your files or workflows is sent.</p>
      <div className={styles.row}>
        <Where status={status} />
        {(status.state === "idle" || status.state === "failed") && (
          <Button variant="secondary" onClick={() => void check()}>Check now</Button>
        )}
        {status.state === "ready" && <Button onClick={() => void restart()}>Restart to update</Button>}
      </div>
      {status.state === "ready" && (
        <>
          {status.notes && <p className={styles.notes}>{status.notes}</p>}
          <p className={styles.muted}>Or it installs the next time you quit Vela.</p>
        </>
      )}
      {error && <p className={styles.error} role="alert">{error}</p>}
    </>
  );
}

function Where({ status }: { status: ReturnType<typeof useUpdates>["status"] }) {
  switch (status.state) {
    case "idle":
      return <span>{status.checkedAt ? `Vela is up to date. Checked ${ago(status.checkedAt)}.` : "Not checked yet."}</span>;
    case "checking":
      return <Spinner label="Checking for updates…" />;
    case "downloading":
      return <Spinner label={`Downloading Vela ${status.version}…`} />;
    case "ready":
      return <span>Vela {status.version} is ready to install.</span>;
    case "installing":
      return <Spinner label={`Installing Vela ${status.version}…`} />;
    case "failed":
      return <span className={styles.error}>{status.message}</span>;
    default:
      return null;
  }
}
