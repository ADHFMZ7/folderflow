# Api contract

How the front end talks to the Rust core. Screens call the `Api` interface in `src/api/api.ts`. In the app, `src/api/tauri.ts` turns each call into a Tauri command; in the browser, tests and previews, `src/api/mock.ts` implements the same behaviour.

JSON field names are camelCase. Tauri maps a command's snake_case parameters to camelCase keys, so `detect(provider_id)` is invoked as `invoke("detect", { providerId })`.

## Commands

| Api method | Command | Arguments | Returns |
| --- | --- | --- | --- |
| `getSettings()` | `get_settings` | none | `LoadedSettings` |
| `updateSettings(change)` | `update_settings` | `change: SettingsChange` | `Settings` |
| `listModelKinds()` | `list_model_kinds` | none | `ModelKind[]` |
| `listProviders()` | `list_providers` | none | `Provider[]` |
| `detect(providerId)` | `detect` | `providerId` | `DetectResult` |
| `connect(providerId, credentials)` | `connect` | `providerId`, `credentials` | `ConnectOutcome` |
| `removeConnection(id)` | `remove_connection` | `id` | `Settings` |
| `listModels(connectionId)` | `list_models` | `connectionId` | `Model[]` |
| `listTemplates()` | `list_templates` | none | `Template[]` |
| `chooseFolder(start?)` | `choose_folder` | `start: string \| null` | `string \| null` |
| `chooseCsv(start?)` | `choose_csv` | `start: string \| null` | `string \| null` |

Workflow commands (`list_workflows`, `get_workflow`, `create_workflow`, `save_workflow`, `delete_workflow`, `validate_workflow`) and the workflow file format are in `docs/workflow-format.md`.

## Rules

- **Rust owns connections.** The front end never writes `connections`. `connect` checks the credentials with the provider, stores the key in the Keychain, saves the connection, and gives every kind without a working default the first model of that kind from the new connection. `removeConnection` deletes the key first, then the connection, and clears defaults that used it.
- **Keys travel one way.** A key goes from the front end to Rust once, inside `connect`. No command returns a key.
- **Partial updates.** `updateSettings` takes only `setupComplete`, `openAtLogin` and `defaults` (which replaces the whole map), applies them under one lock, and returns the result. A default that points at a connection that doesn't exist is rejected with `invalid`.
- **Expected refusals are values.** A rejected key (401 or 403), an unreachable provider, a missing key or a bad address comes back as `{ ok: false, error }` from `connect`, or `{ found: false, reason }` from `detect`.
- **Unexpected provider answers are errors.** A server error, an unreadable reply or a redirect fails with `provider`, from `connect`, `detect` and `listModels` alike. The UI shows the message and lets the user try again, and one unreachable provider never stops the app from opening.
- **Custom servers keep their address.** A connection made by address has an `endpoint` (saved without a trailing slash); other connections have none.
- **Failures are `ApiError`s** with a `code` and a key-free `message`:

  | Code | Meaning |
  | --- | --- |
  | `too_new` | The settings file comes from a newer FolderFlow and was left untouched |
  | `not_found` | Unknown provider or connection |
  | `invalid` | The request breaks a rule, such as a default pointing at a missing connection, or `detect` on a provider that isn't connected by detection |
  | `keychain` | The Keychain refused a request |
  | `provider` | A provider answered in a way that isn't an expected refusal |
  | `io` | Reading or writing a file failed |
  | `conflict` | A save was based on an older revision than the one on disk |

- **Pickers open from Rust.** `choose_folder` and `choose_csv` show the native macOS panel in front of the window (`tauri-plugin-dialog`, used from Rust, so the window needs no dialog permission). They start in `start` when it names an existing folder, or the folder of an existing file, and answer with the chosen path, the home folder written as `~`, or `null` when cancelled. `choose_csv` picks an existing .csv; a new file's path is typed. The mock answers with `MockOptions.chosenFolder` / `chosenCsv` (null is a cancel), or a sample path in previews.
- **Load notices.** `getSettings` returns `notice: { kind: "recovered", backup }` when the settings file was unreadable and was moved aside, and keeps returning it for the rest of the session, so a second call (React Strict Mode, a remount) can't lose it. Dismissing it only hides it in the UI. A file from a newer version fails with `too_new`, and the UI blocks until the user opens a newer FolderFlow.
