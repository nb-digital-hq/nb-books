# Security Policy

## Reporting a vulnerability

**Do not open a public issue.** A public issue is a full disclosure before a fix exists.

Send the report by email to **nbdigitalsas@gmail.com** with a subject that starts exactly like this:

```text
[NB Books][Seguridad] <one-line summary>
```

Example: `[NB Books][Seguridad] Path traversal when importing a ZIP container`.

The mailbox is shared by every NB Digital project and is filtered by that prefix. **A report without the
`[NB Books][Seguridad]` prefix is very likely not to be read.** Once the repository is public, GitHub's private
vulnerability reporting (Security tab, *Report a vulnerability*) is also accepted.

Please include:

1. A description of the problem and the affected component.
2. Steps to reproduce, or a proof of concept.
3. The estimated impact (confidentiality, integrity, availability, privacy).
4. The version (Settings, About) or commit hash.

## What to expect

| Step | Commitment |
|---|---|
| Acknowledgement | Within 7 days |
| Public disclosure | The reporter may disclose 90 days after acknowledgement, fixed or not |
| Assessment and severity | Communicated to the reporter |
| Fix | Developed privately |
| Publication | Fixed release and security advisory, with credit if desired |

The project follows **coordinated disclosure**. The full procedure, severity levels and the supply-chain
compromise procedure are in [`docs/legal/governance/security-policy.md`](docs/legal/governance/security-policy.md)
(Spanish).

## Scope

**In scope:**

* The desktop application: runtime (Rust), interface (Svelte) and the IPC bridge between them.
* The network gate (`src-tauri/src/net.rs`) and any path that could send data off the device without consent.
* The WebAssembly extension sandbox, its permission model and the extension registry review tooling.
* Parsing of untrusted documents: EPUB, PDF, comic archives, ZIP containers, OPDS feeds and Calibre libraries.
* Build, packaging, signing and release infrastructure.

**Out of scope:** third-party operating-system software, third-party extensions not published in the official
registry, and deployments modified by the user.

The threat model (STRIDE), including the limits the project declares it does **not** cover, is in
[`docs/audit/threat-model.md`](docs/audit/threat-model.md).

## Cryptographic scope

| Use | Mechanism |
|---|---|
| Release integrity | `SHA256SUMS` over every artefact, signed with **Ed25519** (`SHA256SUMS.sig`); provenance record per release |
| Extension packages | Author signature with Ed25519, verified by the review tool before publication in the registry |
| Network transport | TLS through the single network gate; plain-text catalogue hosts require their own, separate consent |
| Stored credentials | Operating-system keyring (Secret Service on Linux); never stored in settings, database or logs |
| Data at rest | Not encrypted by the application: it relies on the operating system's disk encryption. See [encryption](docs/architecture/security/encryption.md) |

How to verify a release is described in [releases](docs/developer/deployment/releases.md), section 6.

## Supported versions

Security fixes are released for the latest published version.
