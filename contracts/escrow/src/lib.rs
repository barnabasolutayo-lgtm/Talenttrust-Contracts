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
mod refund;
mod release;
mod ttl;
mod types;

pub use migration::PendingClientMigration;
pub use ttl::PENDING_MIGRATION_TTL_LEDGERS;
pub use types::{
    Contract, ContractStatus, DataKey, Error, Milestone, MilestoneApprovals, ReadinessChecklist,
    ReleaseAuthorization, Reputation,
};

use soroban_sdk::{contract, contractimpl, contracttype, symbol_short, Address, Env, Symbol, Vec};

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
    ProtocolFeeOverflow = 25,
    ProtocolFeeBpsExceedsMaximum = 26,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContractData {
    pub client: Address,
    pub freelancer: Address,
    pub milestones: Vec<i128>,
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

        let timestamp = env.ledger().timestamp();

        env.events().publish(
            (symbol_short!("init"), Symbol::new(&env, "admin_set")),
            (admin.clone(), timestamp),
        );

        let initial_fee_bps: u32 = env
            .storage()
            .persistent()
            .get(&DataKey::ProtocolFeeBps)
            .unwrap_or(0);

        let initial_params: crate::types::GovernedParameters = env
            .storage()
            .persistent()
            .get(&DataKey::GovernedParameters)
            .unwrap_or_default();

        env.events().publish(
            (symbol_short!("init"), Symbol::new(&env, "config")),
            (admin, timestamp, initial_fee_bps, initial_params),
        );

        true
    }

    /// Returns the stored governance admin address, if one has been initialized.
    pub fn get_admin(env: Env) -> Option<Address> {
        env.storage().persistent().get(&DataKey::Admin)
    }

    /// Returns the current mainnet readiness checklist.
    pub fn get_mainnet_readiness_info(env: Env) -> ReadinessChecklist {
        env.storage()
            .persistent()
            .get(&DataKey::ReadinessChecklist)
            .unwrap_or_default()
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

    // -----------------------------------------------------------------------
    // Protocol fee helpers (used by release_milestone in release.rs)
    // -----------------------------------------------------------------------

    /// Returns true if the contract has been initialized.
    fn is_initialized(env: &Env) -> bool {
        env.storage()
            .persistent()
            .get::<_, bool>(&DataKey::Initialized)
            .unwrap_or(false)
    }

    /// Returns the protocol fee in basis points.
    fn get_protocol_fee_bps(env: &Env) -> u32 {
        env.storage()
            .persistent()
            .get::<_, u32>(&DataKey::ProtocolFeeBps)
            .unwrap_or(0)
    }

    /// Maximum allowed protocol fee in basis points (99.99%).
    pub const MAX_PROTOCOL_FEE_BPS: u32 = 10_000;

    /// Calculates the protocol fee for a given amount.
    ///
    /// Uses overflow-safe arithmetic with explicit round-half-up semantics.
    /// The fee is computed as `(amount * fee_bps + 5000 - 1) / 10000` which rounds
    /// toward positive infinity for positive amounts.
    ///
    /// # Arguments
    /// * `env` - The contract environment (for error handling)
    /// * `amount` - The milestone amount (i128, must be positive)
    /// * `fee_bps` - The fee rate in basis points (u32, 1-9999)
    ///
    /// # Returns
    /// The computed fee amount
    ///
    /// # Panics
    /// Panics with `EscrowError::ProtocolFeeOverflow` if `amount * fee_bps` overflows.
    ///
    /// # Security Guarantees
    /// - Fee never equals or exceeds the milestone amount
    /// - Uses checked arithmetic to prevent silent overflow
    /// - Round-half-up ensures deterministic, predictable fee calculation
    fn calculate_protocol_fee(env: &Env, amount: i128, fee_bps: u32) -> i128 {
        // Use u128 widening to handle potential overflow safely
        // amount can be up to i128::MAX, fee_bps up to 9999
        // i128::MAX * 9999 ≈ 1.7 * 10^37 which fits in u128 but could overflow i128
        let amount_u128 = amount as u128;
        let fee_bps_u128 = fee_bps as u128;
        
        // Compute: amount * fee_bps (checked, widened to u128)
        let product = amount_u128
            .checked_mul(fee_bps_u128)
            .unwrap_or_else(|| env.panic_with_error(EscrowError::ProtocolFeeOverflow));
        
        // Round half-up: add (divisor / 2) - 1 before division
        // fee = (amount * fee_bps + 5000 - 1) / 10000 
        // This rounds 0.5 upward (toward positive infinity for positive amounts)
        let rounding = Self::MAX_PROTOCOL_FEE_BPS as u128 / 2; // 5000
        let fee = (product + rounding - 1) / Self::MAX_PROTOCOL_FEE_BPS as u128;
        
        // Cap fee to be strictly less than amount (prevents fee >= amount)
        // This is the final security guarantee: fee < milestone.amount
        fee.min(amount_u128.saturating_sub(1)) as i128
    }

    /// Refunds unreleased milestones back to the client.
    ///
    /// # Arguments
    /// * `env` - The contract environment
    /// * `contract_id` - The contract ID
    /// * `milestone_indices` - Vector of milestone indices to refund
    ///
    /// # Returns
    /// The total amount refunded
    ///
    /// # Errors
    /// * `ContractNotFound` - If contract doesn't exist
    /// * `EmptyRefundRequest` - If milestone_indices is empty
    /// * `DuplicateMilestoneInRefund` - If the same milestone appears multiple times
    /// * `IndexOutOfBounds` - If any milestone index is out of bounds
    /// * `AlreadyReleased` - If any milestone was already released
    /// * `AlreadyRefunded` - If any milestone was already refunded
    /// * `InsufficientFunds` - If contract doesn't have enough balance to refund
    pub fn refund_unreleased_milestones(
        env: Env,
        contract_id: u32,
        milestone_indices: Vec<u32>,
    ) -> i128 {
        // Validate non-empty request
        if milestone_indices.is_empty() {
            env.panic_with_error(Error::EmptyRefundRequest);
        }

        // Check for duplicates
        for i in 0..milestone_indices.len() {
            for j in (i + 1)..milestone_indices.len() {
                if milestone_indices.get(i).unwrap() == milestone_indices.get(j).unwrap() {
                    env.panic_with_error(Error::DuplicateMilestoneInRefund);
                }
            }
        }

        let mut contract: Contract = env
            .storage()
            .persistent()
            .get(&DataKey::Contract(contract_id))
            .unwrap_or_else(|| env.panic_with_error(Error::ContractNotFound));

        // Extend TTL on contract read
        ttl::extend_contract_ttl(&env, contract_id);

        Self::require_not_finalized(&env, contract_id);

        contract.client.require_auth();

        let milestone_key = Symbol::new(&env, "milestones");
        let mut milestones: Vec<Milestone> = env
            .storage()
            .persistent()
            .get(&(DataKey::Contract(contract_id), milestone_key.clone()))
            .unwrap();

        // Extend TTL on milestone read
        ttl::extend_milestone_ttl(&env, contract_id);

        let mut total_refund_amount: i128 = 0;

        // Validate all milestones first
        for idx in milestone_indices.iter() {
            if idx >= milestones.len() {
                env.panic_with_error(Error::IndexOutOfBounds);
            }

            let milestone = milestones.get(idx).unwrap();

            if milestone.released {
                env.panic_with_error(Error::AlreadyReleased);
            }

            if milestone.refunded {
                env.panic_with_error(Error::AlreadyRefunded);
            }

            total_refund_amount += milestone.amount;
        }

        // Check if there's enough balance
        let available_balance =
            contract.funded_amount - contract.released_amount - contract.refunded_amount;
        if available_balance < total_refund_amount {
            env.panic_with_error(Error::InsufficientFunds);
        }

        // Mark milestones as refunded
        for idx in milestone_indices.iter() {
            let mut milestone = milestones.get(idx).unwrap();
            milestone.refunded = true;
            milestones.set(idx, milestone);
        }

        contract.refunded_amount += total_refund_amount;

        // Check if all unreleased milestones are refunded
        let all_refunded_or_released = milestones.iter().all(|m| m.released || m.refunded);
        if all_refunded_or_released {
            let all_refunded = milestones.iter().all(|m| m.refunded);
            if all_refunded {
                contract.status = ContractStatus::Refunded;
            } else {
                // Some released, some refunded
                contract.status = ContractStatus::Completed;
            }
        }

        env.storage().persistent().set(
            &(DataKey::Contract(contract_id), milestone_key),
            &milestones,
        );
        env.storage()
            .persistent()
            .set(&DataKey::Contract(contract_id), &contract);

        // Extend TTL on contract and milestone writes
        ttl::extend_contract_and_milestones_ttl(&env, contract_id);

        total_refund_amount
    }

    // -----------------------------------------------------------------------
    // Protocol fee helpers (used by release_milestone in release.rs)
    // -----------------------------------------------------------------------

    /// Returns true if the contract has been initialized.
    fn is_initialized(env: &Env) -> bool {
        env.storage()
            .persistent()
            .get::<_, bool>(&DataKey::Initialized)
            .unwrap_or(false)
    }

    /// Returns the protocol fee in basis points.
    fn get_protocol_fee_bps(env: &Env) -> u32 {
        env.storage()
            .persistent()
            .get::<_, u32>(&DataKey::ProtocolFeeBps)
            .unwrap_or(0)
    }

    /// Maximum allowed protocol fee in basis points (99.99%).
    pub const MAX_PROTOCOL_FEE_BPS: u32 = 10_000;

    /// Calculates the protocol fee for a given amount.
    ///
    /// Uses overflow-safe arithmetic with explicit round-half-up semantics.
    /// The fee is computed as `(amount * fee_bps + 5000 - 1) / 10000` which rounds
    /// toward positive infinity for positive amounts.
    ///
    /// # Arguments
    /// * `env` - The contract environment (for error handling)
    /// * `amount` - The milestone amount (i128, must be positive)
    /// * `fee_bps` - The fee rate in basis points (u32, 1-9999)
    ///
    /// # Returns
    /// The computed fee amount
    ///
    /// # Panics
    /// Panics with `EscrowError::ProtocolFeeOverflow` if `amount * fee_bps` overflows.
    ///
    /// # Security Guarantees
    /// - Fee never equals or exceeds the milestone amount
    /// - Uses checked arithmetic to prevent silent overflow
    /// - Round-half-up ensures deterministic, predictable fee calculation
    fn calculate_protocol_fee(env: &Env, amount: i128, fee_bps: u32) -> i128 {
        // Use u128 widening to handle potential overflow safely
        // amount can be up to i128::MAX, fee_bps up to 9999
        // i128::MAX * 9999 ≈ 1.7 * 10^37 which fits in u128 but could overflow i128
        let amount_u128 = amount as u128;
        let fee_bps_u128 = fee_bps as u128;
        
        // Compute: amount * fee_bps (checked, widened to u128)
        let product = amount_u128
            .checked_mul(fee_bps_u128)
            .unwrap_or_else(|| env.panic_with_error(EscrowError::ProtocolFeeOverflow));
        
        // Round half-up: add (divisor / 2) - 1 before division
        // fee = (amount * fee_bps + 5000 - 1) / 10000 
        // This rounds 0.5 upward (toward positive infinity for positive amounts)
        let rounding = Self::MAX_PROTOCOL_FEE_BPS as u128 / 2; // 5000
        let fee = (product + rounding - 1) / Self::MAX_PROTOCOL_FEE_BPS as u128;
        
        // Cap fee to be strictly less than amount (prevents fee >= amount)
        // This is the final security guarantee: fee < milestone.amount
        fee.min(amount_u128.saturating_sub(1)) as i128
    }

    /// Retrieves contract information.
    ///
    /// # Arguments
    /// * `env` - The contract environment
    /// * `contract_id` - The contract ID
    ///
    /// # Returns
    /// The contract data
    ///
    /// # Errors
    /// * `ContractNotFound` - If contract doesn't exist
    pub fn get_contract(env: Env, contract_id: u32) -> Contract {
        let contract = env
            .storage()
            .persistent()
            .get(&DataKey::Contract(contract_id))
            .unwrap_or_else(|| env.panic_with_error(Error::ContractNotFound));

        // Extend TTL on contract read
        ttl::extend_contract_ttl(&env, contract_id);

        contract
    }

    /// Retrieves all milestones for a contract.
    ///
    /// # Arguments
    /// * `env` - The contract environment
    /// * `contract_id` - The contract ID
    ///
    /// # Returns
    /// Vector of milestones
    ///
    /// # Errors
    /// * `ContractNotFound` - If contract doesn't exist
    pub fn get_milestones(env: Env, contract_id: u32) -> Vec<Milestone> {
        let milestone_key = Symbol::new(&env, "milestones");
        let milestones = env
            .storage()
            .persistent()
            .get(&(DataKey::Contract(contract_id), milestone_key))
            .unwrap_or_else(|| env.panic_with_error(Error::ContractNotFound));

        // Extend TTL on milestone read
        ttl::extend_milestone_ttl(&env, contract_id);

        milestones
    }

    /// Calculates the refundable balance (funded but not released or refunded).
    ///
    /// # Arguments
    /// * `env` - The contract environment
    /// * `contract_id` - The contract ID
    ///
    /// # Returns
    /// The refundable balance amount
    ///
    /// # Errors
    /// * `ContractNotFound` - If contract doesn't exist
    pub fn get_refundable_balance(env: Env, contract_id: u32) -> i128 {
        let contract: Contract = env
            .storage()
            .persistent()
            .get(&DataKey::Contract(contract_id))
            .unwrap_or_else(|| env.panic_with_error(Error::ContractNotFound));

        // Extend TTL on contract read
        ttl::extend_contract_ttl(&env, contract_id);

        contract.funded_amount - contract.released_amount - contract.refunded_amount
    }

    /// Retrieves approval status for a milestone.
    ///
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
        Self::require_initialized(&env);
        let admin: Address = env.storage().persistent().get(&DataKey::Admin).unwrap();
        admin.require_auth();
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
        Self::require_initialized(&env);
        let admin: Address = env.storage().persistent().get(&DataKey::Admin).unwrap();
        admin.require_auth();
        env.storage().persistent().set(&DataKey::Emergency, &false);
        env.storage().persistent().set(&DataKey::Paused, &false);
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

    /// Cancels a contract if the caller is the client or freelancer and the contract
    /// is in a cancellable state (Created, PartiallyFunded, or Funded).
    ///
    /// # Arguments
    /// * `env` - The contract environment
    /// * `contract_id` - The contract ID
    /// * `caller` - The address of the caller (must be authorized)
    ///
    /// # Returns
    /// `true` if cancellation was successful
    ///
    /// # Errors
    /// * `ContractNotFound` - If contract doesn't exist
    /// * `UnauthorizedRole` - If caller is not client or freelancer
    /// * `InvalidState` - If contract is not in a cancellable state
    /// * `ContractPaused` - If the contract is paused
    ///
    /// # Events
    /// Emits `("cancelled", contract_id)` with payload:
    /// - `caller`: The address that cancelled the contract
    /// - `previous_status`: The status before cancellation
    /// - `timestamp`: Ledger timestamp at cancellation time
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

        let previous_status = contract.status;

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

        env.events().publish(
            (symbol_short!("cancelled"), contract_id),
            (caller, previous_status, env.ledger().timestamp()),
        );

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

    pub fn get_pending_reputation_credits(env: Env, address: Address) -> i128 {
        env.storage()
            .persistent()
            .get(&DataKey::PendingReputationCredits(address))
            .unwrap_or(0)
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

#[cfg(test)]
mod test;
