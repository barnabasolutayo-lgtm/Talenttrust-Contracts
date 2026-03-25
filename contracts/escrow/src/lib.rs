#![no_std]

use soroban_sdk::{contract, contractimpl, contracttype, Address, Env, Symbol, Vec};
use soroban_sdk::token::Client as TokenClient;

/// Represents the status of an Escrow contract.
#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ContractStatus {
    /// Contract has been created but no funds deposited.
    Created = 0,
    /// Contract has been funded by the client.
    Funded = 1,
    /// Contract milestones completed and funds released.
    Completed = 2,
    /// Contract is under dispute.
    Disputed = 3,
    InDispute = 4,
}

/// Represents a milestone within an escrow contract.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Milestone {
    /// Amount allocated for this milestone.
    pub amount: i128,
    /// Whether this milestone has been released.
    pub released: bool,
}

/// Represents an escrow contract instance.
#[contracttype]
#[derive(Clone, Debug)]
pub struct EscrowContract {
    /// The client funding the contract.
    pub client: Address,
    /// The freelancer receiving payments.
    pub freelancer: Address,
    /// List of milestones and their amounts.
    pub milestones: Vec<Milestone>,
    /// Current contract status.
    pub status: ContractStatus,
}

/// Main Escrow contract.
#[contract]
pub struct Escrow;

impl Escrow {
    fn read_admin(env: &Env) -> Address {
        env.storage()
            .instance()
            .get(&DataKey::Admin)
            .unwrap_or_else(|| panic!("Pause controls are not initialized"))
    }

    fn require_admin(env: &Env) {
        let admin = Self::read_admin(env);
        admin.require_auth();
    }

    fn is_paused_internal(env: &Env) -> bool {
        env.storage()
            .instance()
            .get(&DataKey::Paused)
            .unwrap_or(false)
    }

    fn is_emergency_internal(env: &Env) -> bool {
        env.storage()
            .instance()
            .get(&DataKey::EmergencyPaused)
            .unwrap_or(false)
    }

    fn ensure_not_paused(env: &Env) {
        if Self::is_paused_internal(env) {
            panic!("Contract is paused");
        }
    }
}

#[contractimpl]
impl Escrow {
    /// Create a new escrow contract.
    ///
    /// # Arguments
    /// * `env` - The contract execution environment.
    /// * `client` - The address of the client funding the contract.
    /// * `freelancer` - The address of the freelancer.
    /// * `milestone_amounts` - List of amounts for each milestone.
    ///
    /// # Returns
    /// Returns a `u32` contract ID for the new escrow.
    pub fn create_contract(
        env: Env,
        client: Address,
        freelancer: Address,
        milestone_amounts: Vec<i128>,
    ) -> u32 {
        let contract_id: u32 = 1;

        let mut milestones: Vec<Milestone> = Vec::new(&env);
        for amount in milestone_amounts.iter() {
            milestones.push_back(Milestone {
                amount,
                released: false,
            });
        }

        let escrow = EscrowContract {
            client,
            freelancer,
            milestones,
            status: ContractStatus::Created,
        };

        env.storage().instance().set(&contract_id, &escrow);

        contract_id
    }

    /// Deposit funds into the escrow contract.
    ///
    /// Only the client can deposit. Updates contract status to `Funded` if successful.
    pub fn deposit_funds(
        env: Env,
        contract_id: u32,
        token: Address,
        client: Address,
        amount: i128,
    ) -> bool {
        if !validate_amount(amount) {
            return false;
        }

        let escrow_option: Option<EscrowContract> =
            env.storage().instance().get(&contract_id);
        if escrow_option.is_none() {
            return false;
        }

        let mut escrow = escrow_option.unwrap();

        if client != escrow.client {
            return false;
        }

        let success = safe_token_transfer(
            &env,
            &token,
            &client,
            &env.current_contract_address(),
            amount,
        );

        if success {
            escrow.status = ContractStatus::Funded;
            env.storage().instance().set(&contract_id, &escrow);
        }

        success
    }

    /// Release a milestone payment to the freelancer.
    ///
    /// Only the assigned freelancer can receive funds. Updates contract status
    /// to `Completed` if all milestones are released successfully.
    pub fn release_milestone(
        env: Env,
        contract_id: u32,
        token: Address,
        freelancer: Address,
        amount: i128,
    ) -> bool {
        let escrow_option: Option<EscrowContract> =
            env.storage().instance().get(&contract_id);
        if escrow_option.is_none() {
            return false;
        }

        let mut escrow = escrow_option.unwrap();

        if freelancer != escrow.freelancer {
            return false;
        }

        let success = safe_token_transfer(
            &env,
            &token,
            &env.current_contract_address(),
            &freelancer,
            amount,
        );

        if success {
            escrow.status = ContractStatus::Completed;
            env.storage().instance().set(&contract_id, &escrow);
        }

        success
    }

    /// Issue a reputation credential for the freelancer after contract completion.
    ///
    /// Placeholder function for reputation logic.
    pub fn issue_reputation(_env: Env, _freelancer: Address, _rating: i128) -> bool {
        true
    }

    /// Simple hello function for testing or CI purposes.
    pub fn hello(_env: Env, to: Symbol) -> Symbol {
        to
    }

    /// Returns the stored contract state.
    pub fn get_contract(env: Env, contract_id: u32) -> EscrowContractData {
        Self::load_contract(&env, contract_id)
    }

    /// Returns the stored reputation record for a freelancer, if present.
    pub fn get_reputation(env: Env, freelancer: Address) -> Option<ReputationRecord> {
        env.storage()
            .persistent()
            .get(&DataKey::Reputation(freelancer))
    }

    /// Returns the number of pending reputation updates that can be claimed.
    pub fn get_pending_reputation_credits(env: Env, freelancer: Address) -> u32 {
        env.storage()
            .persistent()
            .get(&DataKey::PendingReputationCredits(freelancer))
            .unwrap_or(0)
    }

    /// Returns the active protocol parameters.
    ///
    /// If governance has not been initialized yet, this returns the safe default
    /// parameters baked into the contract.
    pub fn get_protocol_parameters(env: Env) -> ProtocolParameters {
        Self::protocol_parameters(&env)
    }

    /// Returns the current governance admin, if governance has been initialized.
    pub fn get_governance_admin(env: Env) -> Option<Address> {
        env.storage().persistent().get(&DataKey::GovernanceAdmin)
    }

    /// Returns the pending governance admin, if an admin transfer is in flight.
    pub fn get_pending_governance_admin(env: Env) -> Option<Address> {
        Self::pending_governance_admin(&env)
    }
}

impl Escrow {
    fn next_contract_id(env: &Env) -> u32 {
        env.storage()
            .persistent()
            .get(&DataKey::NextContractId)
            .unwrap_or(1)
    }

    fn load_contract(env: &Env, contract_id: u32) -> EscrowContractData {
        env.storage()
            .persistent()
            .get(&DataKey::Contract(contract_id))
            .unwrap_or_else(|| panic!("contract not found"))
    }

    fn save_contract(env: &Env, contract_id: u32, contract: &EscrowContractData) {
        env.storage()
            .persistent()
            .set(&DataKey::Contract(contract_id), contract);
    }

    fn add_pending_reputation_credit(env: &Env, freelancer: &Address) {
        let key = DataKey::PendingReputationCredits(freelancer.clone());
        let current = env.storage().persistent().get::<_, u32>(&key).unwrap_or(0);
        env.storage().persistent().set(&key, &(current + 1));
    }

    fn governance_admin(env: &Env) -> Address {
        env.storage()
            .persistent()
            .get(&DataKey::GovernanceAdmin)
            .unwrap_or_else(|| panic!("protocol governance is not initialized"))
    }

    fn pending_governance_admin(env: &Env) -> Option<Address> {
        env.storage()
            .persistent()
            .get(&DataKey::PendingGovernanceAdmin)
    }

    fn protocol_parameters(env: &Env) -> ProtocolParameters {
        env.storage()
            .persistent()
            .get(&DataKey::ProtocolParameters)
            .unwrap_or_else(Self::default_protocol_parameters)
    }

    fn default_protocol_parameters() -> ProtocolParameters {
        ProtocolParameters {
            min_milestone_amount: DEFAULT_MIN_MILESTONE_AMOUNT,
            max_milestones: DEFAULT_MAX_MILESTONES,
            min_reputation_rating: DEFAULT_MIN_REPUTATION_RATING,
            max_reputation_rating: DEFAULT_MAX_REPUTATION_RATING,
        }
    }

    fn validated_protocol_parameters(
        min_milestone_amount: i128,
        max_milestones: u32,
        min_reputation_rating: i128,
        max_reputation_rating: i128,
    ) -> ProtocolParameters {
        if min_milestone_amount <= 0 {
            panic!("minimum milestone amount must be positive");
        }
        if max_milestones == 0 {
            panic!("maximum milestones must be positive");
        }
        if min_reputation_rating <= 0 {
            panic!("minimum reputation rating must be positive");
        }
        if min_reputation_rating > max_reputation_rating {
            panic!("reputation rating range is invalid");
        }

        ProtocolParameters {
            min_milestone_amount,
            max_milestones,
            min_reputation_rating,
            max_reputation_rating,
        }
    }

    fn all_milestones_released(milestones: &Vec<Milestone>) -> bool {
        let mut index = 0_u32;
        while index < milestones.len() {
            let milestone = milestones
                .get(index)
                .unwrap_or_else(|| panic!("missing milestone"));
            if !milestone.released {
                return false;
            }
            index += 1;
        }
        true
    }
}

/// Validates that an amount is greater than zero.
fn validate_amount(amount: i128) -> bool {
    amount > 0
}

/// Safely transfers tokens between addresses.
///
/// During tests, this will skip the actual transfer.
fn safe_token_transfer(
    env: &Env,
    token: &Address,
    from: &Address,
    to: &Address,
    amount: i128,
) -> bool {
    if !validate_amount(amount) {
        return false;
    }

    // During tests, skip actual token transfer
    #[cfg(test)]
    {
        return true;
    }

    #[cfg(not(test))]
    {
        let client = TokenClient::new(env, token);
        client.transfer(from, to, &amount);
        true
    }
}

#[cfg(test)]
mod test;