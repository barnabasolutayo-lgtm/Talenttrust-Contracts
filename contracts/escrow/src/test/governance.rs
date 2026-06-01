use super::{register_client, setup};
use crate::EscrowError;
use soroban_sdk::{testutils::Address as _, Address, Env};

#[test]
fn initialization_smoke_test() {
    let env = Env::default();
    env.mock_all_auths();
    let client = register_client(&env);

    let admin = Address::generate(&env);
    assert!(client.initialize(&admin));
}

#[test]
fn double_initialize_fails() {
    let env = Env::default();
    env.mock_all_auths();
    let client = register_client(&env);

    let admin = Address::generate(&env);
    client.initialize(&admin);

    let result = client.try_initialize(&admin);
    super::assert_contract_error(result, EscrowError::AlreadyInitialized);
}

#[test]
fn pause_unpause_governance() {
    let env = Env::default();
    env.mock_all_auths();
    let client = register_client(&env);
    let admin = Address::generate(&env);
    client.initialize(&admin);

    assert!(client.pause());
    assert!(client.is_paused());
    assert!(client.unpause());
    assert!(!client.is_paused());
}

#[test]
fn emergency_pause_governance() {
    let env = Env::default();
    env.mock_all_auths();
    let client = register_client(&env);
    let admin = Address::generate(&env);
    client.initialize(&admin);

    assert!(client.activate_emergency_pause());
    assert!(client.is_paused());
    assert!(client.is_emergency());
    
    assert!(client.resolve_emergency());
    assert!(!client.is_emergency());
}

#[test]
fn update_governance_parameters() {
    let env = Env::default();
    env.mock_all_auths();
    let client = register_client(&env);
    let admin = Address::generate(&env);
    client.initialize(&admin);

    assert!(client.update_governance_parameters(&500, &1000000_0000000));
}
