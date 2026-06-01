use soroban_sdk::{testutils::{Address as _, Events}, Address, Env};

use crate::{
    Escrow, EscrowClient, ReadinessChecklist,
};

fn register_client(env: &Env) -> EscrowClient<'_> {
    let id = env.register(Escrow, ());
    EscrowClient::new(env, &id)
}

#[test]
fn fresh_contract_smoke() {
    let env = Env::default();
    env.mock_all_auths();
    let client = register_client(&env);

    let info = client.get_mainnet_readiness_info();
    assert!(info.admin_set);
}

#[test]
fn initialize_sets_admin() {
    let env = Env::default();
    env.mock_all_auths();
    let client = register_client(&env);
    let admin = Address::generate(&env);

    client.initialize(&admin);

    let info = client.get_mainnet_readiness_info();
    assert!(info.admin_set);
}

#[test]
fn set_governed_params_smoke() {
    let env = Env::default();
    env.mock_all_auths();
    let client = register_client(&env);
    let admin = Address::generate(&env);

    client.initialize(&admin);
    assert!(client.set_governed_params(&admin, &1000, &500_000_000_000));
}

#[test]
fn get_mainnet_readiness_info_no_auth_no_events() {
    let env = Env::default();
    let client = register_client(&env);

    let _info = client.get_mainnet_readiness_info();

    let events = env.events().all();
    assert!(events.is_empty());
}
