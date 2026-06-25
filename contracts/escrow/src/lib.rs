#![no_std]
#![allow(clippy::derivable_impls)]
#![allow(clippy::manual_range_contains)]
#![allow(clippy::assertions_on_constants)]
#![allow(clippy::too_many_arguments)]
#![allow(clippy::type_complexity)]
#![allow(clippy::needless_range_loop)]
#![allow(clippy::collapsible_if)]
#![allow(clippy::collapsible_else_if)]
#![allow(clippy::redundant_field_names)]
#![allow(clippy::ptr_arg)]
#![allow(clippy::useless_vec)]
#![allow(clippy::let_and_return)]
#![allow(clippy::inconsistent_digit_grouping)]
#![allow(clippy::int_plus_one)]
#![allow(clippy::duplicated_attributes)]
#![allow(clippy::unreadable_literal)]
#![allow(clippy::redundant_clone)]
#![allow(clippy::bool_assert_comparison)]
#![allow(clippy::needless_borrow)]
#![allow(clippy::clone_on_copy)]
#![allow(clippy::module_inception)]
#![allow(clippy::single_match)]
#![allow(clippy::useless_conversion)]

mod approvals;
mod create_contract;
mod deposit;
mod finalize;
mod governance;
mod migration;
mod refund;
mod release;
mod ttl;
mod types;

pub use migration::PendingClientMigration;
pub use ttl::{ADMIN_ROTATION_MIN_DELAY_LEDGERS, PENDING_MIGRATION_TTL_LEDGERS};
pub use types::{
    Contract, ContractStatus, CONTRACT_SUMMARY_SCHEMA_VERSION, ContractSummary, DataKey,
    DepositMode, Error, GovernedParameters, MAINNET_MAX_TOTAL_ESCROW_PER_CONTRACT_STROOPS,
    MAINNET_PROTOCOL_VERSION, Milestone, MilestoneApprovals, MilestoneSummary, PendingAdminProposal,
    ReadinessChecklist, ReleaseAuthorization, Reputation,
};

use soroban_sdk::{contract, contractimpl, contracterror, contracttype, symbol_short, Address, Env, Symbol, Vec};

#[contract]
pub struct Escrow;

#[contracterror]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EscrowError {
    InvalidParticipant = 1,
    EmptyMilestones = 2,
    InvalidMilestoneAmount = 3,
    InvalidDepositAmount = 4,
    InvalidMilestone = 5,
    ContractNotFound = 6,
    EmptyRefundRequest = 7,
    DuplicateMilestoneInRefund = 8,
    AlreadyReleased = 9,
    AlreadyRefunded = 10,
    InsufficientFunds = 11,
    AlreadyInitialized = 12,
    InsufficientAccumulatedFees = 13,
    NotInitialized = 14,
    UnauthorizedRole = 15,
    ContractPaused = 16,
    EmergencyActive = 17,
    InvalidState = 18,
    InvalidRating = 19,
    SelfRating = 20,
    ReputationAlreadyIssued = 21,
    NotCompleted = 22,
    FreelancerMismatch = 23,
    InvalidStatusTransition = 24,
    AlreadyFinalized = 25,
    InvalidProtocolParameters = 26,
    GovernanceNotInitialized = 27,
    PotentialOverflow = 28,
    AccountingInvariantViolated = 29,
    TimelockNotElapsed = 30,
}

#[contractimpl]
impl Escrow {
    /// Hello-world style function for testing and CI.
    pub fn hello(_env: Env, to: Symbol) -> Symbol {
        to
    }

    /// Initializes the escrow contract with the operational admin.
    ///
    /// This call is single-use and stores the admin address for future
    /// admin-gated entrypoints such as `withdraw_protocol_fees`.
    pub fn initialize(env: Env, admin: Address) -> bool {
        if env
            .storage()
            .persistent()
            .get::<_, bool>(&DataKey::Initialized)
            .unwrap_or(false)
        {
            env.panic_with_error(EscrowError::AlreadyInitialized);
        }

        admin.require_auth();
        env.storage().persistent().set(&DataKey::Initialized, &true);
        env.storage().persistent().set(&DataKey::Admin, &admin);

        let mut checklist: ReadinessChecklist = env
            .storage()
            .persistent()
            .get(&DataKey::ReadinessChecklist)
            .unwrap_or_default();
        checklist.initialized = true;
        env.storage()
            .persistent()
            .set(&DataKey::ReadinessChecklist, &checklist);

        env.events().publish(
            (symbol_short!("init"), Symbol::new(&env, "admin_set")),
            (admin.clone(), env.ledger().timestamp()),
        );

        true
    }

    /// Returns the stored governance admin address, if one has been initialized.
    pub fn get_admin(env: Env) -> Option<Address> {
        env.storage().persistent().get(&DataKey::Admin)
    }

    /// Returns the current mainnet readiness checklist.
    ///
    /// Read-only and requires no authorization. The `caps_set`, `protocol_version`,
    /// and `max_escrow_total_stroops` fields are always populated from compile-time
    /// constants regardless of storage state.
    pub fn get_mainnet_readiness_info(env: Env) -> ReadinessChecklist {
        let mut checklist: ReadinessChecklist = env
            .storage()
            .persistent()
            .get(&DataKey::ReadinessChecklist)
            .unwrap_or_default();
        // Always reflect compile-time constants, never rely on stored values for these.
        checklist.caps_set = MAINNET_MAX_TOTAL_ESCROW_PER_CONTRACT_STROOPS > 0;
        checklist.protocol_version = MAINNET_PROTOCOL_VERSION;
        checklist.max_escrow_total_stroops = MAINNET_MAX_TOTAL_ESCROW_PER_CONTRACT_STROOPS;
        checklist
    }

    /// Approves a milestone for release.
    ///
    /// Records the approval in temporary storage with TTL expiry.
    /// Approvals automatically expire after PENDING_APPROVAL_TTL_LEDGERS.
    ///
    /// # Arguments
    /// * `env` - The contract environment
    /// * `contract_id` - The contract ID
    /// * `caller` - The address of the caller (must be authorized)
    /// * `milestone_index` - The index of the milestone to approve
    ///
    /// # Returns
    /// `true` if approval was recorded successfully
    ///
    /// # Errors
    /// * `ContractNotFound` - If contract doesn't exist
    /// * `InvalidState` - If contract is not in Funded state
    /// * `IndexOutOfBounds` - If milestone index is invalid
    /// * `MilestoneAlreadyReleased` - If milestone was already released
    /// * `UnauthorizedRole` - If caller is not authorized to approve
    /// * `AlreadyApproved` - If caller has already approved this milestone
    ///
    /// # Security
    /// - Caller must be authenticated
    /// - Only authorized parties can approve based on ReleaseAuthorization mode
    /// - Approvals expire via TTL and are auto-evicted
    /// - Duplicate approvals are rejected
    pub fn approve_milestone_release(
        env: Env,
        contract_id: u32,
        caller: Address,
        milestone_index: u32,
    ) -> bool {
        approvals::approve_milestone(&env, contract_id, milestone_index, &caller)
            .unwrap_or_else(|e| env.panic_with_error(e))
    }

    /// Retrieves contract information.
    pub fn get_contract(env: Env, contract_id: u32) -> Contract {
        let contract = env
            .storage()
            .persistent()
            .get(&DataKey::Contract(contract_id))
            .unwrap_or_else(|| env.panic_with_error(Error::ContractNotFound));
        ttl::extend_contract_ttl(&env, contract_id);
        contract
    }

    /// Retrieves all milestones for a contract.
    pub fn get_milestones(env: Env, contract_id: u32) -> Vec<Milestone> {
        let milestone_key = Symbol::new(&env, "milestones");
        let milestones = env
            .storage()
            .persistent()
            .get(&(DataKey::Contract(contract_id), milestone_key))
            .unwrap_or_else(|| env.panic_with_error(Error::ContractNotFound));
        ttl::extend_milestone_ttl(&env, contract_id);
        milestones
    }

    /// Returns funded minus released minus refunded for `contract_id`.
    pub fn get_refundable_balance(env: Env, contract_id: u32) -> i128 {
        let contract: Contract = env
            .storage()
            .persistent()
            .get(&DataKey::Contract(contract_id))
            .unwrap_or_else(|| env.panic_with_error(Error::ContractNotFound));
        ttl::extend_contract_ttl(&env, contract_id);
        contract.funded_amount - contract.released_amount - contract.refunded_amount
    }

    /// Retrieves approval status for a milestone.
    /// Returns None if approvals have expired or don't exist.
    ///
    /// # Arguments
    /// * `env` - The contract environment
    /// * `contract_id` - The contract ID
    /// * `milestone_index` - The milestone index
    ///
    /// # Returns
    /// Optional MilestoneApprovals struct
    pub fn get_milestone_approvals(
        env: Env,
        contract_id: u32,
        milestone_index: u32,
    ) -> Option<MilestoneApprovals> {
        let approval_key = DataKey::MilestoneApprovals(contract_id, milestone_index);
        env.storage().temporary().get(&approval_key)
    }

    // -----------------------------------------------------------------------
    // Pause / unpause
    // -----------------------------------------------------------------------

    pub fn pause(env: Env) -> bool {
        Self::require_initialized(&env);
        let admin: Address = env.storage().persistent().get(&DataKey::Admin).unwrap();
        admin.require_auth();
        env.storage().persistent().set(&DataKey::Paused, &true);
        true
    }

    pub fn unpause(env: Env) -> bool {
        Self::require_initialized(&env);
        if env
            .storage()
            .persistent()
            .get::<_, bool>(&DataKey::Emergency)
            .unwrap_or(false)
        {
            env.panic_with_error(EscrowError::EmergencyActive);
        }
        let admin: Address = env.storage().persistent().get(&DataKey::Admin).unwrap();
        admin.require_auth();
        env.storage().persistent().set(&DataKey::Paused, &false);
        true
    }

    pub fn is_paused(env: Env) -> bool {
        env.storage()
            .persistent()
            .get(&DataKey::Paused)
            .unwrap_or(false)
    }

    // -----------------------------------------------------------------------
    // Emergency pause
    // -----------------------------------------------------------------------

    pub fn activate_emergency_pause(env: Env) -> bool {
        if env
            .storage()
            .persistent()
            .get::<_, bool>(&DataKey::Initialized)
            .unwrap_or(false)
        {
            let admin: Address = env.storage().persistent().get(&DataKey::Admin).unwrap();
            admin.require_auth();
        }
        env.storage().persistent().set(&DataKey::Emergency, &true);
        env.storage().persistent().set(&DataKey::Paused, &true);
        let mut checklist: ReadinessChecklist = env
            .storage()
            .persistent()
            .get(&DataKey::ReadinessChecklist)
            .unwrap_or_default();
        checklist.emergency_controls_enabled = true;
        env.storage()
            .persistent()
            .set(&DataKey::ReadinessChecklist, &checklist);
        true
    }

    pub fn resolve_emergency(env: Env) -> bool {
        if env
            .storage()
            .persistent()
            .get::<_, bool>(&DataKey::Initialized)
            .unwrap_or(false)
        {
            let admin: Address = env.storage().persistent().get(&DataKey::Admin).unwrap();
            admin.require_auth();
        }
        env.storage().persistent().set(&DataKey::Emergency, &false);
        env.storage().persistent().set(&DataKey::Paused, &false);
        let mut checklist: ReadinessChecklist = env
            .storage()
            .persistent()
            .get(&DataKey::ReadinessChecklist)
            .unwrap_or_default();
        checklist.emergency_controls_enabled = true;
        env.storage()
            .persistent()
            .set(&DataKey::ReadinessChecklist, &checklist);
        true
    }

    pub fn is_emergency(env: Env) -> bool {
        env.storage()
            .persistent()
            .get(&DataKey::Emergency)
            .unwrap_or(false)
    }

    // -----------------------------------------------------------------------
    // Cancel contract
    // -----------------------------------------------------------------------

    pub fn cancel_contract(env: Env, contract_id: u32, caller: Address) -> bool {
        let mut contract: Contract = env
            .storage()
            .persistent()
            .get(&DataKey::Contract(contract_id))
            .unwrap_or_else(|| env.panic_with_error(Error::ContractNotFound));
        ttl::extend_contract_ttl(&env, contract_id);

        if caller != contract.client && caller != contract.freelancer {
            env.panic_with_error(Error::UnauthorizedRole);
        }

        match contract.status {
            ContractStatus::Created | ContractStatus::PartiallyFunded | ContractStatus::Funded => {}
            _ => env.panic_with_error(Error::InvalidState),
        }

        caller.require_auth();
        contract.status = ContractStatus::Cancelled;
        env.storage()
            .persistent()
            .set(&DataKey::Contract(contract_id), &contract);
        ttl::extend_contract_ttl(&env, contract_id);
        true
    }

    // -----------------------------------------------------------------------
    // Reputation
    // -----------------------------------------------------------------------

    pub fn issue_reputation(
        env: Env,
        contract_id: u32,
        caller: Address,
        freelancer: Address,
        rating: i128,
    ) -> bool {
        let contract: Contract = env
            .storage()
            .persistent()
            .get(&DataKey::Contract(contract_id))
            .unwrap_or_else(|| env.panic_with_error(Error::ContractNotFound));
        ttl::extend_contract_ttl(&env, contract_id);

        if caller != contract.client {
            env.panic_with_error(Error::UnauthorizedRole);
        }
        if freelancer != contract.freelancer {
            env.panic_with_error(Error::FreelancerMismatch);
        }

        if rating < 1 || rating > 5 {
            env.panic_with_error(EscrowError::InvalidRating);
        }

        if contract.status != ContractStatus::Completed {
            env.panic_with_error(EscrowError::NotCompleted);
        }

        if env
            .storage()
            .persistent()
            .get::<_, bool>(&DataKey::ReputationIssued(contract_id))
            .unwrap_or(false)
        {
            env.panic_with_error(EscrowError::ReputationAlreadyIssued);
        }

        if contract.client == contract.freelancer {
            env.panic_with_error(EscrowError::SelfRating);
        }

        caller.require_auth();
        env.storage()
            .persistent()
            .set(&DataKey::ReputationIssued(contract_id), &true);

        let pending_key = DataKey::PendingReputationCredits(contract.freelancer.clone());
        let pending: i128 = env.storage().persistent().get(&pending_key).unwrap_or(0);
        env.storage().persistent().set(&pending_key, &(pending - 1));

        let rep_key = DataKey::Reputation(contract.freelancer.clone());
        let mut rep: types::Reputation =
            env.storage().persistent().get(&rep_key).unwrap_or_default();
        rep.completed_contracts += 1;
        rep.total_rating += rating;
        rep.last_rating = rating;
        env.storage().persistent().set(&rep_key, &rep);

        true
    }

    pub fn get_reputation(env: Env, address: Address) -> Option<types::Reputation> {
        env.storage()
            .persistent()
            .get(&DataKey::Reputation(address))
    }

    /// Returns the freelancer's average rating scaled to basis points (×10 000),
    /// or `None` if no reputation record exists or no contracts have been completed.
    ///
    /// # Scaling
    /// `result = total_rating * 10_000 / completed_contracts`
    ///
    /// A raw rating of 5 on a single contract returns `50_000` (5.0000 on a
    /// 1–5 scale).  Clients divide by `10_000` to recover the decimal value.
    ///
    /// Checked arithmetic is used throughout; division by zero is impossible
    /// because `None` is returned whenever `completed_contracts == 0`.
    pub fn get_average_rating(env: Env, address: Address) -> Option<i128> {
        /// Basis-point scaling factor (×10 000 preserves four decimal places).
        const SCALE: i128 = 10_000;

        let rep: types::Reputation = env
            .storage()
            .persistent()
            .get(&DataKey::Reputation(address))?;

        if rep.completed_contracts == 0 {
            return None;
        }

        rep.total_rating
            .checked_mul(SCALE)
            .and_then(|scaled| scaled.checked_div(rep.completed_contracts))
    }

    pub fn get_pending_reputation_credits(env: Env, address: Address) -> i128 {
        env.storage()
            .persistent()
            .get(&DataKey::PendingReputationCredits(address))
            .unwrap_or(0)
    }

    // -----------------------------------------------------------------------
    // Governed parameters
    // -----------------------------------------------------------------------

    /// Sets protocol fee (basis points) and maximum escrow total per contract.
    ///
    /// Requires initialization. Only the stored admin may call this.
    /// `fee_bps` must be ≤ 10 000; `max_escrow_total_stroops` must be > 0.
    /// Sets `governed_params_set` in the readiness checklist on success.
    pub fn set_governed_params(
        env: Env,
        caller: Address,
        fee_bps: u32,
        max_escrow_total_stroops: i128,
    ) -> bool {
        Self::require_initialized(&env);
        let admin: Address = env.storage().persistent().get(&DataKey::Admin).unwrap();
        if caller != admin {
            env.panic_with_error(EscrowError::UnauthorizedRole);
        }
        caller.require_auth();
        if fee_bps > 10_000 || max_escrow_total_stroops <= 0 {
            env.panic_with_error(EscrowError::InvalidProtocolParameters);
        }
        let params = GovernedParameters { protocol_fee_bps: fee_bps, max_escrow_total_stroops };
        env.storage()
            .persistent()
            .set(&DataKey::GovernedParameters, &params);
        let mut checklist: ReadinessChecklist = env
            .storage()
            .persistent()
            .get(&DataKey::ReadinessChecklist)
            .unwrap_or_default();
        checklist.governed_params_set = true;
        env.storage()
            .persistent()
            .set(&DataKey::ReadinessChecklist, &checklist);
        true
    }

    /// Returns the current governed parameters, if set.
    pub fn get_governed_parameters(env: Env) -> Option<GovernedParameters> {
        env.storage()
            .persistent()
            .get(&DataKey::GovernedParameters)
    }

    // -----------------------------------------------------------------------
    // Governance: treasury rotation with timelock
    // -----------------------------------------------------------------------

    /// Propose a new admin. Stores the pending admin proposal (with the
    /// current ledger sequence for timelock enforcement) and emits an event.
    ///
    /// Requires initialization; current admin must authorize.
    pub fn propose_governance_admin(env: Env, proposed: Address) -> bool {
        Self::propose_governance_admin_impl(env, proposed)
    }

    /// Accept a pending admin proposal. The proposed admin must authorize.
    ///
    /// Fails with `TimelockNotElapsed` if fewer than
    /// `ADMIN_ROTATION_MIN_DELAY_LEDGERS` have elapsed since the proposal.
    pub fn accept_governance_admin(env: Env) -> bool {
        Self::accept_governance_admin_impl(env)
    }

    /// Return the currently pending admin address, if any.
    pub fn get_pending_governance_admin(env: Env) -> Option<Address> {
        Self::get_pending_governance_admin_impl(env)
    }

    /// Return the current admin address.
    pub fn get_governance_admin(env: Env) -> Option<Address> {
        Self::get_governance_admin_impl(env)
    }

    // -----------------------------------------------------------------------
    // Internal helpers
    // -----------------------------------------------------------------------

    fn require_initialized(env: &Env) {
        if !env
            .storage()
            .persistent()
            .get::<_, bool>(&DataKey::Initialized)
            .unwrap_or(false)
        {
            env.panic_with_error(EscrowError::NotInitialized);
        }
    }
}

/// Subtracts `b` from `a`, returning `None` if the result would underflow.
pub fn safe_subtract_amounts(a: i128, b: i128) -> Option<i128> {
    a.checked_sub(b)
}

#[cfg(test)]
mod test;
