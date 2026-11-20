# Contributing to NB Books

Thank you for your interest. This file is the entry point; the complete guide, in Spanish, is in
[`docs/developer/contributing.md`](docs/developer/contributing.md). If you work with an AI coding agent, point it
to [AGENTS.md](AGENTS.md): it contains the same rules in a machine-friendly form.

## Before writing code

1. Read the [product philosophy](docs/design/product-philosophy.md) and the
   [Constitution](docs/architecture/constitution.md). A technically correct contribution that contradicts them
   will be declined.
2. Check the features already rejected, with their reasons, in
   [reading experience](docs/design/reading-experience/README.md).
3. Look for an existing issue. If there is none, open one before implementing.
4. For changes with architectural impact, open a proposal before coding. See the
   [decision process](docs/legal/governance/decision-process.md).

This protects your time: nothing is more frustrating than a large pull request declined for product-direction
reasons that could have been discussed first.

## Licence and provenance

NB Books is released under the **Apache License 2.0**. Your contribution is licensed under the same terms and
**you keep your copyright**: there is no copyright assignment and no contributor licence agreement.

The only requirement is the Developer Certificate of Origin (DCO): every commit carries a `Signed-off-by` line.

```bash
git commit -s -m "fix(library): corregir orden de la cuadrícula al importar"
```

If you forgot to sign off, `git rebase --signoff origin/main` fixes the commits on your branch.

## Workflow

1. Fork the repository and create a branch: `feat/short-description` or `fix/short-description`.
2. Make atomic commits following [Conventional Commits](docs/developer/commit-convention.md). The `commit-msg`
   hook rejects messages that do not comply.
3. Run `just verify` (or `pnpm verify` plus the Rust checks listed in [AGENTS.md](AGENTS.md)) before opening the
   pull request.
4. Open the pull request against `main` and complete the template.

For a first contribution, follow [`docs/developer/first-pr.md`](docs/developer/first-pr.md) step by step.

## What is not accepted

* Third-party code without a compatible licence or without attribution.
* Features that remove technical protection measures from protected works.
* Telemetry, analytics, advertising or any data transfer without explicit consent.
* Copyrighted files used as test material.
* Large refactorings without prior agreement.
* New dependencies without a justification of cost, licence and maintenance
  ([dependency policy](docs/architecture/dependency-policy.md)).

## Conduct and security

All interactions are governed by the [Code of Conduct](CODE_OF_CONDUCT.md). **Do not report vulnerabilities as
public issues**; see [SECURITY.md](SECURITY.md).
