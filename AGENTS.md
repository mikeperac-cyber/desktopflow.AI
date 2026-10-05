# DeskFlow AI contributor guide

Read these files before changing the product:

1. `README.md` for current scope and commands.
2. `docs/ARCHITECTURE.md` for security boundaries and module ownership.
3. `docs/ROADMAP.md` for phase boundaries and acceptance criteria.
4. `docs/TESTING.md` before claiming a feature is complete.
5. `docs/DESIGN_SYSTEM.md` before changing visible UI.
6. `docs/AI_PROVIDER_CHECKPOINT.md` before Phase 5 provider work.

Keep AI output untrusted, retain the Rust-side safety boundary, and never add arbitrary shell execution to the normal action vocabulary. Do not implement a later phase on top of a failing `npm run check` or native build.
