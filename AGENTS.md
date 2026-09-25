# AGENTS.md

FolderFlow (working name) is a macOS app, built with Tauri, for file workflows built as a node graph. The UI is React in `src/`, and the core is Rust in `src-tauri/`.

## Testing

Work test-first: write the test, watch it go red for the expected reason, then write the code that turns it green. A bug fix starts with a test that goes red on the bug.

These areas must never regress. Any change touching them adds or updates tests for the failure, not only the happy path:

- **Keys**: never in settings, logs, errors, workflow files or the front end; only sent to their own provider.
- **User files**: no data loss, every action undoable, nothing outside the granted folders.
- **Where files go**: content goes only to the provider the step uses.
- **Consent**: destructive actions and Ask me steps wait for the user.

Read `TESTING.md` before writing tests, adding a test layer, or changing file actions, credentials, providers, validation or workflow execution.
