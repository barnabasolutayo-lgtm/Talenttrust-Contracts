# Escrow Access Control Enforcement

This document describes role checks currently enforced in
`contracts/escrow/src/lib.rs`.

## Implemented Checks

- `initialize(admin)` requires `admin.require_auth()` and can run only once.
- `pause`, `unpause`, `activate_emergency_pause`, and `resolve_emergency`
  require the stored admin's authorization.
- `create_contract(client, freelancer, milestone_amounts, deposit_mode)`
  requires `client.require_auth()`.
- `issue_reputation(contract_id, caller, freelancer, rating)` requires
  `caller.require_auth()` and `caller == contract.client`.
- `cancel_contract(contract_id, caller)` requires `caller.require_auth()` and
  the caller must be the stored client or freelancer.

## Current Release Caveat

`release_milestone(contract_id, milestone_index)` does not authenticate a
caller in the current implementation. It enforces pause state, contract
existence, milestone bounds, duplicate-release prevention, and available funded
balance only.

## Not Implemented

Approval-based release authorization, arbiter release authorization, and
multi-party release modes are not live entrypoints. Treat any approval or
arbiter release design as planned until implemented and tested.
