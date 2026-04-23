#![no_std]

use soroban_sdk::{contract, contracterror, contractimpl, contracttype, Address, Env, Symbol, Vec};

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
    ContractNotFinalized = 6,
    UnauthorizedClient = 7,
    NoLeftoverFunds = 8,
    AlreadyWithdrawn = 9,
    InsufficientBalance = 10,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContractData {
    pub client: Address,
    pub freelancer: Address,
    pub milestones: Vec<i128>,
    pub deposited: i128,
    pub released: i128,
    pub refunded: i128,
    pub finalized: bool,
    pub leftover_withdrawn: bool,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LeftoverWithdrawnEvent {
    pub contract_id: u32,
    pub client: Address,
    pub amount: i128,
}

#[contracttype]
#[derive(Clone)]
enum DataKey {
    NextId,
    Contract(u32),
}

#[contractimpl]
impl Escrow {
    /// Hello-world style function for testing and CI.
    pub fn hello(_env: Env, to: Symbol) -> Symbol {
        to
    }

    pub fn create_contract(
        env: Env,
        client: Address,
        freelancer: Address,
        milestones: Vec<i128>,
    ) -> u32 {
        if client == freelancer {
            env.panic_with_error(EscrowError::InvalidParticipant);
        }
        if milestones.is_empty() {
            env.panic_with_error(EscrowError::EmptyMilestones);
        }

        for amount in milestones.iter() {
            if amount <= 0 {
                env.panic_with_error(EscrowError::InvalidMilestoneAmount);
            }
        }

        let id = env
            .storage()
            .persistent()
            .get::<_, u32>(&DataKey::NextId)
            .unwrap_or(0);

        let data = ContractData {
            client,
            freelancer,
            milestones,
            deposited: 0,
            released: 0,
            refunded: 0,
            finalized: false,
            leftover_withdrawn: false,
        };

        env.storage()
            .persistent()
            .set(&DataKey::Contract(id), &data);
        env.storage().persistent().set(&DataKey::NextId, &(id + 1));

        id
    }

    pub fn deposit_funds(env: Env, contract_id: u32, amount: i128, from: Address) -> bool {
        if amount <= 0 {
            env.panic_with_error(EscrowError::InvalidDepositAmount);
        }
        
        let mut data = env.storage().persistent().get::<_, ContractData>(&DataKey::Contract(contract_id))
            .unwrap_or_else(|| env.panic_with_error(EscrowError::InvalidMilestone));
        
        if data.client != from {
            env.panic_with_error(EscrowError::UnauthorizedClient);
        }
        
        data.deposited += amount;
        env.storage().persistent().set(&DataKey::Contract(contract_id), &data);
        true
    }

    pub fn release_milestone(env: Env, contract_id: u32, milestone_index: u32, from: Address) -> bool {
        let mut data = env.storage().persistent().get::<_, ContractData>(&DataKey::Contract(contract_id))
            .unwrap_or_else(|| env.panic_with_error(EscrowError::InvalidMilestone));
        
        if data.client != from {
            env.panic_with_error(EscrowError::UnauthorizedClient);
        }
        
        if milestone_index >= data.milestones.len() as u32 {
            env.panic_with_error(EscrowError::InvalidMilestone);
        }
        
        let milestone_amount = data.milestones.get(milestone_index as u32).unwrap();
        if data.released + milestone_amount > data.deposited {
            env.panic_with_error(EscrowError::InsufficientBalance);
        }
        
        data.released += milestone_amount;
        env.storage().persistent().set(&DataKey::Contract(contract_id), &data);
        true
    }

    pub fn finalize_contract(env: Env, contract_id: u32, from: Address) -> bool {
        let mut data = env.storage().persistent().get::<_, ContractData>(&DataKey::Contract(contract_id))
            .unwrap_or_else(|| env.panic_with_error(EscrowError::InvalidMilestone));
        
        if data.client != from {
            env.panic_with_error(EscrowError::UnauthorizedClient);
        }
        
        if data.finalized {
            return true; // Already finalized
        }
        
        data.finalized = true;
        env.storage().persistent().set(&DataKey::Contract(contract_id), &data);
        true
    }

    pub fn withdraw_leftover(env: Env, contract_id: u32, from: Address) -> i128 {
        let mut data = env.storage().persistent().get::<_, ContractData>(&DataKey::Contract(contract_id))
            .unwrap_or_else(|| env.panic_with_error(EscrowError::InvalidMilestone));
        
        // Strict invariants
        if data.client != from {
            env.panic_with_error(EscrowError::UnauthorizedClient);
        }
        
        if !data.finalized {
            env.panic_with_error(EscrowError::ContractNotFinalized);
        }
        
        if data.leftover_withdrawn {
            env.panic_with_error(EscrowError::AlreadyWithdrawn);
        }
        
        let leftover = data.deposited - data.released - data.refunded;
        
        if leftover <= 0 {
            env.panic_with_error(EscrowError::NoLeftoverFunds);
        }
        
        // Update state
        data.leftover_withdrawn = true;
        env.storage().persistent().set(&DataKey::Contract(contract_id), &data);
        
        // Emit event
        let event = LeftoverWithdrawnEvent {
            contract_id,
            client: data.client,
            amount: leftover,
        };
        env.events().publish(("LeftoverWithdrawn", "withdraw"), event);
        
        leftover
    }
}

#[cfg(test)]
mod test;
