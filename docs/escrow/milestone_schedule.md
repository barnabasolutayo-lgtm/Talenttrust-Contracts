# Escrow Milestone Scheduling

Milestone due dates, schedules, and deadline-driven timeout behavior are not
implemented in `contracts/escrow/src/lib.rs`.

The current milestone model is a vector of positive `i128` amounts plus
per-milestone release flags. Release validates only milestone existence,
duplicate-release state, available funded balance, and pause/emergency state.

Scheduling documentation should be restored only when the corresponding storage
fields and public entrypoints are implemented.
