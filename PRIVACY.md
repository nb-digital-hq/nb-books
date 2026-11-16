# Privacy Statement

This statement describes, in technical and binding terms, what NB Books does with your data. It is enforced by
the code and by automated checks that run on every change; where a guarantee depends on a check, the check is
named. The detailed Spanish reference is [docs/audit/privacy.md](docs/audit/privacy.md).

## 1. Data sovereignty

* **Everything is local.** The catalogue, annotations, notes, reading progress, search index, settings and
  downloaded models live in your user directories (`~/.local/share/nb-books`, `~/.config/nb-books`,
  `~/.cache/nb-books`, or the `XDG_*` equivalents). There is no server operated by the project that receives them.
* **Your files are referenced, not copied.** Library folders are read; the application does not write, rename or
  move files inside them.
* **No account.** No feature requires registration, an email address, a licence key or a sign-in.
* **Portability.** The whole library state can be exported to an open format and imported on another machine.

## 2. Zero telemetry

NB Books contains **no** analytics, usage tracking, crash reporting, advertising identifiers or remote logging.
The module named `telemetry` writes local log files only, and it redacts content: logs never contain file paths,
document titles, authors, excerpts or annotations. This is enforced by `scripts/check-registro.mjs` on every
change.

The diagnostics bundle (Settings, Export diagnostics) is generated only when you ask for it, is redacted in the
same way, and is never sent automatically.

## 3. Network access

Every outgoing connection passes through one module, `src-tauri/src/net.rs`. Any HTTP client used elsewhere
fails the build checks (`scripts/check-network.mjs`). The gate enforces, in order:

1. **Offline mode.** When it is on (Settings, Network and privacy), no connection is opened at all.
2. **Per-host, per-purpose consent.** Authorising a host for one purpose does not authorise it for another.

| Purpose | What is sent | When |
|---|---|---|
| Catalogue (OPDS) | Requests to the catalogue you added | When you browse it |
| Catalogue in plain text | Same, without TLS; separate consent that states it | When you browse it |
| Metadata | Title or identifier of the work being looked up | When you request covers or metadata |
| Look-up | The selected word or passage | When you use the encyclopedia or translator |
| Update check | Installed version and platform | When enabled |
| Extension registry | Nothing about what is installed | When checking for withdrawn extensions |
| Remote speech | The paragraph being read aloud | Only with an online voice you configured |
| Model download | Nothing from your library | When you download a model |
| External AI provider | The fragments a function needs and your request | Only with a provider you configured |
| Extension | Whatever that extension sends | Only if you granted that extension network access |

Local AI models run on your CPU; inference never leaves the device unless you configure an external provider.

## 4. Secrets

API keys and tokens for optional services are stored in the operating-system keyring, never in settings files,
the database or logs.

## 5. Extensions

Extensions run in a WebAssembly sandbox and can only use the capabilities they declare and you grant. Network
access for an extension is a separate consent, attributed to that extension.

## 6. Institutional use

This application does not report usage, progress or annotations to any institution. Institutional services, if
any are deployed, are separate products under their own agreements and cannot read local data through this
application.

## 7. Changes

Any change to this statement is made in the same change as the code it describes, and is visible in the
repository history.
