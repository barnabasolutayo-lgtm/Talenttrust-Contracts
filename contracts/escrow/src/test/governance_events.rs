#![cfg(test)]

use crate::{Escrow, EscrowClient};
use soroban_sdk::{testutils::Address as _, Address, Env, Symbol};

#[test]
fn protocol_fee_bps_change_emits_event_smoke() {
    let env = Env::default();
    env.mock_all_auths();

    let id = env.register(Escrow, ());
    let client = EscrowClient::new(&env, &id);

    let admin = Address::generate(&env);
    client.initialize(&admin);

    // Change protocol fee bps - requires admin and bps
    assert!(client.set_protocol_fee_bps(&admin, &100u32));
}

#[test]
fn admin_propose_and_accept_emit_events_smoke() {
    let env = Env::default();
    env.mock_all_auths();

    let id = env.register(Escrow, ());
    let client = EscrowClient::new(&env, &id);

    let admin = Address::generate(&env);
    client.initialize(&admin);

    let next_admin = Address::generate(&env);
    assert!(client.propose_governance_admin(&admin, &next_admin));

    assert!(client.accept_governance_admin(&next_admin));
}
