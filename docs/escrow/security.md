<<<<<<< feature/contracts-27-contract-ownership-transfer
# Escrow Security Notes

## Security Objectives

- Preserve a single authoritative client address for funding, releases, and migration management.
- Prevent accidental or one-sided client identity transfers.
- Prevent overfunding and duplicate milestone releases.
- Keep failure cases explicit and easy to review.

## Threat Scenarios and Mitigations

## 1. Accidental migration to the wrong address

Threat:
A client enters an incorrect replacement address and unintentionally loses control.

Mitigations:
- Migration is not immediate.
- The proposed address must explicitly confirm.
- The current client must explicitly finalize after confirmation.
- The current client may cancel before finalization.

Residual risk:
- If the wrong address is entered and that address confirms, the current client can still finalize incorrectly. This is reduced, not eliminated, by the explicit two-party handshake.

## 2. Stale approval reuse or silent migration replacement

Threat:
An old pending migration approval could be reused for a different destination.

Mitigations:
- Only one pending migration is allowed at a time.
- A pending request must be cancelled or finalized before a new one can be created.
- Finalization checks the stored current-client snapshot and proposed-client confirmation state.

## 3. Client/freelancer role collapse

Threat:
The same address could control both sides of the escrow relationship.

Mitigations:
- Contract creation rejects `client == freelancer`.
- Migration rejects any proposal that would set the client to the freelancer address.

## 4. Escrow overfunding

Threat:
Deposits larger than the milestone total could trap or mis-account funds.

Mitigations:
- Deposits must be positive.
- Cumulative funding cannot exceed the total milestone amount.
- Overflow checks guard aggregate accounting.

## 5. Duplicate or invalid milestone releases

Threat:
Funds could be released twice or for a non-existent milestone.

Mitigations:
- Releases require full funding first.
- The target milestone index must exist.
- Each milestone has an immutable `released` flag once paid.

## Operational Assumptions

- Soroban address authorization is trusted to authenticate `require_auth` calls.
- Contract storage TTL management is not part of this change and must be handled operationally if long-lived contracts are expected.
- Reputation issuance remains a placeholder and is not security-critical to the migration flow.

## Review Checklist

- Confirm every privileged state transition uses the stored client address.
- Confirm migration cannot finalize before proposed-client confirmation.
- Confirm the pending migration record is deleted on cancel and finalize.
- Confirm milestone totals are the only source of truth for maximum funding.
- Confirm completed contracts reject new migration requests.
=======
# Escrow Pause/Emergency Threat Model

## Scope

This model covers pause and emergency controls in `contracts/escrow/src/lib.rs`.

## Assumptions

- The admin key is securely managed.
- Soroban address authentication behaves as expected.
- Off-chain operators monitor incidents and invoke controls quickly.

## Threat Scenarios and Mitigations

1. Unauthorized pause/unpause/emergency calls.
Mitigation: `require_admin` gate with address auth on all control endpoints.

2. Re-initialization to seize control.
Mitigation: `initialize` is single-use and returns `AlreadyInitialized` on repeat calls.

3. Partial recovery from emergency state.
Mitigation: `unpause` returns `EmergencyActive` while emergency flag is set.

4. State-changing execution during incident containment.
Mitigation: all critical mutating endpoints check `ensure_not_paused`.

## Residual Risks

- Admin key compromise can still misuse pause controls.
- No timelock/multi-sig enforced in this contract version.
- Emergency actions are not event-logged in this baseline implementation.

## Recommended Next Hardening Steps

1. Move admin to a multi-sig account.
2. Add role separation for `pauser` and `resolver`.
3. Add on-chain event emission for pause state transitions.
4. Add optional time-delayed unpause for high-severity incidents.
>>>>>>> main
