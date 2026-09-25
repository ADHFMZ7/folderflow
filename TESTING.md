# Testing

FolderFlow runs unattended on people's own files, holds their API keys, and puts nondeterministic models in the loop. Tests exist to make the bad outcomes impossible, not to reach a coverage number. We work test-first, and we spend the most effort where a bug would cost the most.

## Test-driven development

Every behaviour change follows red, green, refactor:

1. **Red.** Write a test for the behaviour and run it. It fails, and it fails for the reason you expect. A test that passes before the code exists is testing nothing.
2. **Green.** Write the least code that makes it pass.
3. **Refactor.** Clean up with the tests green, then run them again.

A bug fix starts the same way: first a test that goes red on the bug, then the fix. The test stays as the regression guard.

The test and the code it covers ship in the same pull request, and CI must be green before merging.

## Critical risks

These are the outcomes FolderFlow must never cause. Code in these areas is written test-first without exception, and its tests cover the attacks and accidents, not only the happy path.

| Risk | Must hold | Tests to write |
| --- | --- | --- |
| **Leaking keys** | API keys live only in the macOS Keychain and are sent only to the provider they belong to. | A known key string never appears in saved settings, workflow files, logs, error messages, crash output, or anything returned to the front end. A request to provider A never carries provider B's key. Deleting a connection removes its key. |
| **Losing or corrupting files** | No action destroys data the user didn't agree to lose, and every action can be undone. | Rename, move and write in real temp directories: name collisions keep both files, writes are atomic, a crash mid-action leaves the original intact, and undo restores the exact names, locations and contents. Property tests over random action sequences: undoing all of them restores the starting tree. |
| **Acting outside the chosen folder** | Workflows touch only the folders and outputs the user granted. | Paths with `..`, symlinks, absolute paths and case tricks are all refused. An action whose target leaves the granted area fails before touching disk. |
| **Sending files to the wrong place** | A file's contents go only to the provider its step is set to use. | With a local model chosen, no request leaves the Mac. Changing a step's provider changes where its content goes, and nothing else does. |
| **Acting on bad data** | Actions only use values that passed validation. | Extract output that is missing a field, has the wrong type, or is malformed stops the run or sends it to review. It never reaches Rename, Move or Add row. |
| **Acting without consent** | Destructive actions and Ask me steps wait for the user. | Delete and overwrite stay off until enabled. A run paused at Ask me does nothing further until answered, and Skip performs no action. |
| **Running away** | Each file is processed once, and the app never feeds itself. | A file still being written isn't picked up. The same file isn't processed twice, including after a restart. Files the app writes don't trigger its own workflows. |

When you change code in one of these areas, add or update its tests even if the change looks unrelated to the risk.

## Test layers

| Layer | Covers | How |
| --- | --- | --- |
| **Core logic** (Rust) | File actions, the undo journal, confinement, intake and deduplication, graph execution, validation | Unit tests next to the code, integration tests in `src-tauri/tests/`. Real files in temp directories, never real user folders. Property tests where inputs vary widely. |
| **Contracts** | The `Api` interface between the UI and Rust; the saved workflow format | One behaviour suite runs against both the mock api and the real one, so they can't drift. Save and load round-trips, including files from older format versions. |
| **Screens** | What a person sees and does | Vitest and Testing Library, next to the component as `*.test.tsx`. Query by role and visible text, as a user would. Render the whole app through `renderApp()` in `src/test/render.tsx`, which uses the mock api with no delays. |
| **Model handling** | What the app does with model output | A scripted fake model returns chosen outputs: valid, malformed, empty, refused, too slow. These tests are deterministic and gate merges. |
| **Model quality** | How well real models classify and extract | An eval suite runs real models on sample files and reports accuracy per model over time. It runs on demand and never gates merges, because model output varies between runs. |

Tauri's end-to-end driver doesn't support macOS, so the layers above carry the weight. Keep the UI thin and the logic in testable Rust.

## Rules

- **Test behaviour through public interfaces.** A test that breaks when code is refactored without changing behaviour is testing the implementation.
- **Fake only real boundaries**: network, models and the clock. Use the real file system, in temp directories.
- **Keep tests deterministic.** Wait for what the user would see (`findBy…`), never for a fixed time. A flaky test is a bug: fix it or delete it the same day. Retrying until it passes hides a race.
- **Keep the main suite tight.** Unit and screen tests run in seconds locally. Evals and other slow suites run separately.
- **Treat coverage as a signal.** A gap in a critical-risk area matters; a gap in a placeholder page doesn't.
