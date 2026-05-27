# Escrow Storage TTL

`contracts/escrow/src/ttl.rs` exports TTL constants for planned pending approval
and migration records.

No current public entrypoint in `contracts/escrow/src/lib.rs` writes pending
approval or pending migration records, so there is no live TTL-managed approval
or migration flow to document yet.

Persistent live records are summarized in [state-persistence.md](state-persistence.md).
