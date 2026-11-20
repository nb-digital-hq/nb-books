# AGENTS.md: operating contract for automated contributors

This file is the working contract of the repository for any coding agent (Claude, Gemini, Codex, Cursor,
Copilot or others) and for humans who want the same rules in one place. It applies **without anyone having to
repeat it**. Tool-specific configuration is never versioned in this repository and must never duplicate or contradict this
file.

The project documentation is written in Spanish; this file, the root community files and the issue templates
are in English. Code identifiers and comments are in Spanish.

---

## 1. The rule that governs all others

**`docs/` is the single source of truth.** If the code and `docs/` disagree, there is a defect: either the
code is fixed or the document is changed through the decision process. A contradiction is never left alive.

A change with architectural or user-facing impact **is not finished** until the affected document reflects it,
with its status label up to date, in the same pull request (Constitution, rule 5).

## 2. Before writing code, in this order

| Step | Document | What to look for |
|---|---|---|
| 1 | [Constitution](docs/architecture/constitution.md) | The absolute rules. Nothing may break them |
| 2 | [ADR index](docs/adr/README.md) | Why things are the way they are |
| 3 | [Architecture](docs/architecture/README.md) | Contracts of the module you are about to touch |
| 4 | [Reading experience](docs/design/reading-experience/README.md) | Mandatory when the change touches product or interface |

The reading-experience document records what people do while reading, what nine reference readers offer, and
**which features were rejected and why**, with sources. A rejected feature is not reopened without new evidence;
the evidence is added to that document with its source, and only then is the topic rediscussed.

If the work depends on a decision that has not been taken, **do not start**: raise the decision first.

## 3. Repository architecture

```text
docs/              Single source of truth (Spanish). Start at docs/README.md
  adr/             Architecture Decision Records (000-template.md, 001-...)
  architecture/    Domain, C4 diagrams, SQLite schema, IPC contracts, AI, voice, extensions
  audit/           Zero-telemetry guarantees, STRIDE threat model, supply chain, performance
  design/          UX flows, accessibility, themes (including the paper theme), icons, tokens
  developer/       Local setup, code structure, coding standards, test suites, packaging
  institution/     Information for institutional decision makers, IT and compliance
  legal/           Licensing, trademark, copyright, data protection, governance
  user/            End-user manual
src/               Frontend: SvelteKit 3 + Svelte 5, static adapter
  lib/core/ipc/    The only access point to the runtime
  lib/bindings/    Types generated from Rust with ts-rs. Never edited by hand
  lib/ui/          Design system and components. Colours come from the Material 3 scheme
  lib/ui/vistas/   Home, Library, Discover, Study, Settings, Import
  lib/ui/lector/   Reader: commands, state, modes, split view and panels
  lib/voz/         Read-aloud orchestration on the interface side
  lib/i18n/        Spanish, English and Portuguese catalogues
src-tauri/         Runtime: Rust + Tauri 2, one module per service
  src/db/          Persistence. catalog.rs is the typed face of the schema
  lector/          The reading engine crate, shared with NB Books Mobile: Locator (the universal
                   position unit), EPUB, PDF and comic providers, sanitiser, ZIP reader, error codes
  src/reader/      Reading session and saved progress on top of the engine
  src/library.rs   Content-addressed identity, library roots and relative paths
  src/browse.rs    Browsing; sorting and filtering happen in SQL, not in the interface
  src/search.rs    Full-text index with a derived-storage budget
  src/ai/          Local embeddings, local generation, optional external providers
  src/voice/       Speech Dispatcher, neural voice (Kokoro) and prepared audio
  src/lookup/      Dictionary (StarDict), encyclopedia and translation behind one contract
  src/extensions/  WebAssembly sandbox, manifest, store, permissions, contribution points
  src/net.rs       **The only network exit.** Nothing leaves through any other path
  src/telemetry.rs Local logging with mandatory redaction. No remote telemetry exists
  src/settings.rs  Preferences. Secrets go to the system keyring, never here
  src/portability.rs Export and import of the whole library
  src/bin/         Release tooling behind the `herramientas` feature (review, sign, measure)
  migrations/      Embedded SQL. A published migration is never edited
  wit/             WIT world exposed to extensions
  tests/           Integration, chaos and binary smoke tests
extensiones/       First-party extensions and the adversarial extension bench
scripts/           Guardians: documentation, design system, network egress, error contract,
                   bridge drift, licences, packaging and provenance
e2e/               Playwright walkthroughs (flows, accessibility, performance)
```

## 4. Commands

A [`justfile`](justfile) unifies the entry points. Every recipe delegates to the underlying `pnpm` script or
`cargo` command, so the guardians have a single definition.

| Command | Purpose |
|---|---|
| `just setup` | Install dependencies, Git hooks, the Playwright browser and the PDF engine |
| `just dev` | Run the desktop application (`pnpm tauri dev`) |
| `just test` | Frontend unit tests, interface walkthroughs and Rust tests |
| `just lint` / `just fmt` | Check or apply formatting and lints |
| `just verify` | The full definition of done (see section 7) |
| `just audit` | Network egress, log redaction, path handling, extension capabilities, licences |
| `just docs` | Documentation integrity |
| `just bindings` | Regenerate and compare IPC types and the extension schema |
| `just package-linux` | Build `.deb`, `.rpm`, AppImage and `.tar.gz` |

`pnpm verify` needs the Rust toolchain (bindings are regenerated with `cargo` to compare, notices read
`cargo metadata`) and the Playwright browser (`pnpm exec playwright install chromium`, once).

## 5. Non-negotiable rules

They are not debated in review. The **Mechanism** column says what enforces each rule today. Writing
"automatic" where it is not would be exactly the kind of claim this file forbids.

| Rule | Where it lives | Mechanism |
|---|---|---|
| No error travels as a string. Canonical, stable codes | `src-tauri/lector/src/error.rs` | Automatic: `pnpm verify` and pre-commit reject `Result<_, String>` on the public surface; a test pins the codes |
| No log line contains paths, user names or document titles | `telemetry::redact_path`, `redact_text`, `telemetry::redactar_linea`, `scripts/check-registro.mjs` | Automatic: a log field that names a path, title, author or text must be redacted, measured or justified, and every line is redacted again on its way to the file, which also covers what an error logged with `%error` carries |
| No position is stored as an integer. Always a Locator | `reader::Locator`, [ADR-004](docs/adr/004-reader-locator-model.md) | Automatic: the database rejects an anchor that is not a selector object, and a test walks the schema |
| No component calls the IPC bridge outside `$core/ipc` | Biome `noRestrictedImports` | Automatic, including `.svelte` files |
| No write transaction is opened deferred | `scripts/check-transacciones.mjs`, [storage](docs/architecture/storage.md) section 1.1 | Automatic. Under WAL a deferred transaction that reads and then writes gets `SQLITE_BUSY` immediately, ignoring `busy_timeout` |
| No bridge response carries a row count chosen by the library size | `scripts/check-listas.mjs`, [IPC contracts](docs/architecture/ipc-contracts.md) rule 6 | Automatic: a command returning a collection takes `limit`, or declares its bound with `Cota: <which>` |
| No bridge command runs on the window thread | `scripts/check-comandos.mjs`, IPC contracts rule 5 | Automatic. Exceptions (window, native dialog, menu, tray) must be declared and used |
| No component writes a literal colour, removes focus, duplicates a breakpoint or relies on a mouse-only gesture | `scripts/check-ui.mjs`, [ADR-012](docs/adr/012-ui-paradigm.md) | Automatic; Svelte accessibility warnings fail with `--fail-on-warnings` |
| No media is played from the application's own URL scheme | `scripts/check-ui.mjs`, [performance budgets](docs/architecture/performance-budgets.md) 4.3.1 | Automatic: WebKitGTK does not play it, Chromium does |
| Nothing animates while idle; permanent chrome over scrolling content is opaque | `e2e/rendimiento.spec.ts`, [UI conventions](docs/design/ui-conventions.md) section 12 | Automatic |
| No `:hover` moves the box of the element receiving it | `scripts/check-ui.mjs` | Automatic, also measured in `e2e/rendimiento.spec.ts` |
| The startup bundle stays under 250 KB and lazy chunks stay lazy | `scripts/check-paquete.mjs` | Automatic in `pnpm verify` (needs a build, so not in pre-commit) |
| Bridge types are generated, not written | `ts-rs` to `src/lib/bindings/` | Automatic: `pnpm check:bindings` |
| No host capability is offered to an extension without a permission behind it | `src-tauri/wit/nb-extension.wit`, `scripts/check-capacidades.mjs` | Automatic |
| The registry policy never promises a check that does not exist | [extension policy](docs/legal/governance/extension-policy.md), `scripts/check-politica.mjs` | Automatic: each requirement names its mechanism and the mechanism cites the requirement |
| The published extension-manifest schema is generated from the table that validates it | `extensions/manifest.rs` to `docs/architecture/api/nb-extension.schema.json` | Automatic: `scripts/check-esquema.mjs` also runs the schema and the host validator over the same examples |
| Every external path is canonicalised and bounded | `security::resolve_within`, `scripts/check-rutas.mjs` | Automatic: any `PathBuf`/`&Path` parameter, any text parameter named as a path, and any such field inside a deserialised object |
| Derived data never outweighs the library it describes | [performance budgets](docs/architecture/performance-budgets.md) 4.4, `search::PAUSA_PRESUPUESTO` | Runtime: 20 % by default (30 or 40 % if chosen); the indexer stops at the budget and publishes why |
| No native binary or model weight enters the repository | `.gitignore` and pre-commit | Automatic |
| The binary run in development and the packaged one are the same | `Cargo.toml` (`default-run`), `tauri.conf.json` (`mainBinaryName`), `tests/identidad_del_binario.rs` | Automatic: `cargo test` |
| A failing component cannot bring down another | Event bus and chaos suite | Automatic |
| No console error in any interface walkthrough | `e2e/flujo.spec.ts` (`afterEach`) | Automatic |
| Nothing leaves the device without explicit consent | `src-tauri/src/net.rs`, [privacy](docs/audit/privacy.md) | Automatic: `scripts/check-network.mjs` rejects any HTTP client outside `net.rs`; the gate checks offline mode and per-host, per-purpose consent |
| No emoji or letter used as an icon; titles in title case | `scripts/check-ui.mjs`, [icons](docs/design/icons.md) | Automatic |
| No runtime capability left without an interface | `scripts/check-puente.mjs` | Automatic: registered commands are crossed with the bridge and with interface calls |
| No painted class without a CSS rule; no backdrop blur on permanent chrome; no `will-change` | `scripts/check-ui.mjs` | Automatic |
| No platform-bound code without declaring it | `scripts/check-plataforma.mjs` | Automatic |
| No new dependency without justifying cost, licence and maintenance | [dependency policy](docs/architecture/dependency-policy.md) | Automatic for the licence (`pnpm check:notices`, `cargo deny`); cost and maintenance are justified in the pull request |

## 6. Privacy and zero telemetry

These rules are absolute and apply to every line of code an agent writes:

* **No telemetry, analytics, crash reporting or usage tracking** of any kind, including "anonymous" or opt-out
  variants. `telemetry.rs` writes local log files only; it never sends anything.
* **Every outgoing connection goes through `src-tauri/src/net.rs`.** Using `reqwest`, `ureq`, a raw socket or
  any HTTP client elsewhere fails `pnpm verify`. A new reason to contact the network is a new `Purpose`, with
  its own consent text shown to the user.
* **Offline mode is respected first.** With it on, no code path may open a connection.
* **Secrets go to the system keyring** (`secrets.rs`), never to settings, logs or the database.
* **User content never reaches logs**: paths, titles, authors, excerpts, covers and annotations are redacted or
  measured.
* **No account, sign-in or licence check** is required to use any feature of this application.
* Test corpora with copyrighted material are referenced through the `NB_BOOKS_CORPUS` environment variable,
  are mounted as referenced (never managed) libraries, are never written to, and nothing derived from them
  (paths, titles, fragments, covers) enters the repository or the logs. Tests that need them skip themselves when
  the variable is unset; the simulator that replaces them must preserve variety (distinct paths, covers and
  lengths), because a bench that collapses the measured dimension turns every measurement into a confirmation.

## 7. Verification

Run before declaring anything finished, without being asked:

```bash
just verify
# equivalent to:
pnpm verify
cd src-tauri
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings   # warnings are errors
cargo test
```

A change that does not pass this is not finished, even if it works. `pnpm verify` never builds in release mode
and never packages: a change touching packaging, the bundle or native libraries is also checked with
`just package-linux` (the AppImage requires `NO_STRIP=1`, see
[Linux deployment](docs/developer/deployment/linux.md) section 1.1).

Definition of done for any change:

1. The acceptance criterion is met and another person can check it.
2. There are tests for the added logic, and one regression test per fixed defect. Tests are named after the
   property they verify, not the function they call.
3. Verification is green.
4. If the change crosses the IPC bridge, the generated bindings are current.
5. If it has architectural or user-facing impact, `docs/` is updated in the same change.
6. If it affects the interface, full keyboard navigation works.
7. If it introduces a component with a health state, its chaos-suite scenario exists.

## 8. Keeping `docs/` in sync

* Each fact has one owning document; others link to it. Duplicated facts drift.
* Every technical document declares its status on its first lines (**Implemented**, **Partially implemented**,
  **Design**, **Proposal**). Describing as existing something that does not exist is worse than not documenting it.
* The structure of `docs/` is fixed: `adr`, `architecture`, `audit`, `design`, `developer`, `institution`,
  `legal`, `user`, `assets`. Do not create new top-level folders; moving a document means updating every
  reference to it in code comments, scripts and other documents in the same change.
* `pnpm check:docs` rejects broken links, empty or duplicated files, emoji, machine-specific absolute paths,
  documents without a level-one title and table rows that swallowed the next block.
* Documentation describes the product as it is. Development history, personal names and internal anecdotes do
  not belong in it; the reasoning behind a decision does, in impersonal form.

## 9. Language and style

* Spanish for code identifiers, comments, documentation in `docs/` and commit messages; English for the root
  community files, issue templates and this file. Professional register, **no emoji** anywhere.
* Comments explain **why**, not what. When a decision is not obvious, the comment says what was discarded and
  why.
* Naming follows [naming](docs/developer/coding-standards/naming.md).

## 10. Commits

```bash
git commit -s -m "feat(library): añadir importación por lotes"
```

* Conventional Commits; the `commit-msg` hook rejects anything else.
* `-s` is mandatory: Developer Certificate of Origin.
* The body explains **why** and what was verified, not the list of files.
* Agents add a `Co-Authored-By:` trailer identifying the tool that produced the change.

## 11. What to do without asking

* Run verification; fix formatting, lints and failing tests.
* Write tests for added logic and a regression test for each fixed defect.
* Update the affected `docs/` document and its status label.
* Fix a small defect found along the way, if it is in the same scope.

## 12. What requires asking

* Approving or changing an ADR.
* Adding a dependency or changing a licence.
* Publishing, pushing to a remote, or any outward-facing action.
* Anything that contradicts this file or the Constitution.
* Any change that sends data off the device or widens the extension capability surface.

## 13. One agent on the working tree

**Two agentic tools must never write to this repository at the same time.** Uncoordinated agents on the same
tree have overwritten and truncated documents silently. If modifications appear that you do not recognise:

```bash
git status --short                     # did anything drift without you touching it?
node scripts/check-docs.mjs            # is any document truncated or corrupted?
git show HEAD:<path>                   # compare with the committed version
git restore --source=HEAD <path>       # restore; never trust the copy on disk
```

The reference is always `HEAD`, never the disk.
