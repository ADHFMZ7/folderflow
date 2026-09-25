// First launch: three pages in one card. Finishing marks setup complete, which
// is what makes Root show the main window.

import { useState } from "react";
import { useSettings } from "../../settings/SettingsProvider";
import { Button, ProgressDots, Toggle } from "../../ui";
import { ModelSetup } from "../models/ModelSetup";
import styles from "./FirstLaunch.module.css";

const STEPS = ["welcome", "model", "background"] as const;

export function FirstLaunch() {
  const { settings, update } = useSettings();
  const [step, setStep] = useState(0);
  const name = STEPS[step];
  const hasModel = settings.connections.length > 0;

  const next = () => (step < STEPS.length - 1 ? setStep(step + 1) : update({ setupComplete: true }));

  return (
    <main className={styles.screen}>
      <section className={styles.card} aria-labelledby="setup-title">
        <div className={styles.body}>
          {name === "welcome" && <Welcome />}
          {name === "model" && (
            <>
              <h2 id="setup-title">Connect a model</h2>
              <p className={styles.lead}>Some steps use AI models. Connect at least one provider; you can add more or change this later in Settings.</p>
              <ModelSetup />
            </>
          )}
          {name === "background" && (
            <>
              <h2 id="setup-title">Keep workflows running</h2>
              <p className={styles.lead}>Workflows run while FolderFlow is open. When you close the window it keeps running in the menu bar.</p>
              <Toggle label="Open FolderFlow when I log in" checked={settings.openAtLogin} onChange={(openAtLogin) => update({ openAtLogin })} />
            </>
          )}
        </div>

        <footer className={styles.footer}>
          <ProgressDots count={STEPS.length} current={step} />
          <span className={styles.spacer} />
          {step > 0 && <Button variant="secondary" onClick={() => setStep(step - 1)}>Back</Button>}
          {name === "model" && !hasModel && <Button variant="secondary" onClick={next}>Skip for now</Button>}
          <Button onClick={next} disabled={name === "model" && !hasModel}>
            {name === "welcome" ? "Get started" : name === "background" ? "Finish" : "Continue"}
          </Button>
        </footer>
      </section>
    </main>
  );
}

function Welcome() {
  return (
    <>
      <div className={styles.icon} aria-hidden>⤓</div>
      <h1 id="setup-title">Welcome to FolderFlow</h1>
      <p className={styles.lead}>Decide once what should happen to your files. FolderFlow does it every time a file arrives.</p>
      <ul className={styles.points}>
        <li><strong>Pick a folder</strong><span>Downloads, Desktop, anywhere you choose</span></li>
        <li><strong>Build the steps</strong><span>Drag steps onto a canvas, or describe them in words</span></li>
        <li><strong>Try it first</strong><span>See what would happen on a real file before turning it on</span></li>
      </ul>
    </>
  );
}
