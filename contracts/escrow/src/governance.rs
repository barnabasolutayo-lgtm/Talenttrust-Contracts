use crate::{DataKey, EscrowError};
use soroban_sdk::{symbol_short, Address, Env, Symbol};

#[soroban_sdk::contractimpl]
impl crate::Escrow {
    pub fn set_protocol_fee_bps(env: Env, new_bps: u32) -> bool {
        if !env.storage().persistent().get::<_, bool>(&DataKey::Initialized).unwrap_or(false) {
            env.panic_with_error(EscrowError::NotInitialized);
        }
        let admin: Address = env.storage().persistent().get(&DataKey::Admin).unwrap_or_else(|| env.panic_with_error(EscrowError::NotInitialized));
        admin.require_auth();

        let old_bps: u32 = env.storage().persistent().get(&DataKey::ProtocolFeeBps).unwrap_or(0u32);
        env.storage().persistent().set(&DataKey::ProtocolFeeBps, &new_bps);

        env.events().publish(
            (Symbol::new(&env, "protocol_fee_bps"),),
            (old_bps, new_bps, admin.clone(), env.ledger().timestamp()),
        );
        true
    }

    pub fn propose_governance_admin(env: Env, proposed: Address) -> bool {
        if !env.storage().persistent().get::<_, bool>(&DataKey::Initialized).unwrap_or(false) {
            env.panic_with_error(EscrowError::NotInitialized);
        }
        let admin: Address = env.storage().persistent().get(&DataKey::Admin).unwrap_or_else(|| env.panic_with_error(EscrowError::NotInitialized));
        admin.require_auth();
        env.storage().persistent().set(&DataKey::PendingAdmin, &proposed);
        env.events().publish(
            (symbol_short!("admin"), Symbol::new(&env, "proposed")),
            (admin, proposed.clone(), env.ledger().timestamp()),
        );
        true
    }

    pub fn accept_governance_admin(env: Env) -> bool {
        if !env.storage().persistent().get::<_, bool>(&DataKey::Initialized).unwrap_or(false) {
            env.panic_with_error(EscrowError::NotInitialized);
        }
        let pending: Address = env.storage().persistent().get(&DataKey::PendingAdmin).unwrap_or_else(|| env.panic_with_error(EscrowError::InvalidState));
        pending.require_auth();

        let old_admin: Address = env.storage().persistent().get(&DataKey::Admin).unwrap_or_else(|| env.panic_with_error(EscrowError::NotInitialized));
        env.storage().persistent().set(&DataKey::Admin, &pending);
        env.storage().persistent().remove(&DataKey::PendingAdmin);

        env.events().publish(
            (symbol_short!("admin"), Symbol::new(&env, "accepted")),
            (old_admin, pending.clone(), env.ledger().timestamp()),
        );
        true
    }

    pub fn get_pending_governance_admin(env: Env) -> Option<Address> {
        env.storage().persistent().get(&DataKey::PendingAdmin)
    }

    pub fn get_governance_admin(env: Env) -> Option<Address> {
        env.storage().persistent().get(&DataKey::Admin)
    }
}
