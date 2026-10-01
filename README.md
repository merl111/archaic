# Archaic

A planned native Matrix desktop client built with Rust and CUI for Linux, Windows and macOS.

This repository currently contains the researched implementation plan, not a working Matrix client.

- [Implementation plan](docs/PLAN.md): architecture, phases, risk decisions and release criteria.
- [Element parity inventory](docs/PARITY.md): versioned feature families, acceptance checks and source evidence.
- [CUI work required](docs/CUI-GAPS.md): reusable framework changes, kept in the separate CUI repository.
- [Design and localization](docs/DESIGN.md): desktop layouts, interaction rules and multilingual behavior.
- [Machine-readable inventory](docs/parity.json) and [upstream feature/settings index](docs/upstream-index.json).

CUI is a separate library repository at `../cui` in this workspace. Archaic must consume its public Rust binding and C ABI; it must not copy private backend code or become part of CUI's examples.
