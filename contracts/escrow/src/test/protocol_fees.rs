#![cfg(test)]

use soroban_sdk::{testutils::Address as _, Address, Env, vec};
use crate::{Escrow, EscrowClient, ReleaseAuthorization};

fn register_client(env: &Env) -> EscrowClient<'_> {
    let id = env.register(Escrow, ());
    EscrowClient::new(env, &id)
}

#[test]
fn test_fee_accrual_and_withdrawal_smoke() {
    let env = Env::default();
    env.mock_all_auths();
    
    let admin = Address::generate(&env);
    let client = register_client(&env);
    client.initialize(&admin);

    let client_addr = Address::generate(&env);
    let freelancer_addr = Address::generate(&env);
    
    let milestones = vec![&env, 1000_i128, 2500_i128, 3333_i128];
    let id = client.create_contract(&client_addr, &freelancer_addr, &None, &milestones, &ReleaseAuthorization::ClientOnly);

    client.deposit_funds(&id, &client_addr, &6833_i128);

    assert!(client.approve_milestone_release(&id, &client_addr, &0));
    assert!(client.release_milestone(&id, &0, &client_addr));
    
    assert!(client.approve_milestone_release(&id, &client_addr, &1));
    assert!(client.release_milestone(&id, &1, &client_addr));
    
    assert!(client.approve_milestone_release(&id, &client_addr, &2));
    assert!(client.release_milestone(&id, &2, &client_addr));

    let destination = Address::generate(&env);
    // withdraw_protocol_fees from lib.rs is (env, admin, destination, amount)
    assert!(client.withdraw_protocol_fees(&admin, &destination, &684_i128));
}

#[test]
#[should_panic]
fn test_unauthorized_withdrawal() {
    let env = Env::default();
    env.mock_all_auths();
    
    let admin = Address::generate(&env);
    let client = register_client(&env);
    client.initialize(&admin);
    
    let fake_admin = Address::generate(&env);
    let destination = Address::generate(&env);
    
    client.withdraw_protocol_fees(&fake_admin, &destination, &100_i128);
}
