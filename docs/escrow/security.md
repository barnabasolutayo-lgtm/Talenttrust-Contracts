# Security Assumptions & Threat Model

This document outlines the security assumptions, potential threat vectors, and mitigations incorporated into the Escrow contract.

## Authorization & Access Control
- **Mitigation Strategy**: The contract relies explicitly on Soroban's `caller.require_auth()` for mutable state changes.
- **Scenario**: An attacker attempts to approve a milestone.
- **Defense**: The `ReleaseAuthorization` enum enforces rigid checks against the caller's address. `ClientOnly` disallows even an assigned Arbiter from approving milestones.

## Ledger Timestamps
- **Scenario**: Premature release via timestamp manipulation.
- **Defense**: Time measurements for approvals utilize `env.ledger().timestamp()`, which safely syncs to Stellar consensus bounds.

## Arithmetic Bounds
- **Scenario**: Integer overflow or underflow leading to locked funds or infinite mints.
- **Defense**: Escrow milestones rely on `i128` limits. No active inflation is coded. Deposits must exactly match the sum of milestone allocations to prevent over or under-collateralization.

## Trust Implications
- Providing an `arbiter` address essentially hands over partial or full funds custody depending on the chosen `ReleaseAuthorization`. Clients should evaluate the reputation of arbiters before initiating a `ClientAndArbiter`, `ArbiterOnly`, or `MultiSig` escrow instance.
