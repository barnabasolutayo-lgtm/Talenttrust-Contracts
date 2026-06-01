use soroban_sdk::{testutils::Address as _, vec, Address, Env};

use crate::{Escrow, EscrowClient, ReleaseAuthorization};

fn register_client(env: &Env) -> EscrowClient<'_> {
    let id = env.register(Escrow, ());
    EscrowClient::new(env, &id)
}

fn default_milestones(env: &Env) -> soroban_sdk::Vec<i128> {
    vec![env, 100_0000000_i128, 200_0000000_i128, 300_0000000_i128]
}

#[test]
#[should_panic(expected = "HostError: Error(Contract, #1)")]
fn rejects_client_equals_freelancer() {
    let env = Env::default();
    env.mock_all_auths();
    let client = register_client(&env);
    let admin = Address::generate(&env);
    client.initialize(&admin);
    let same_party = Address::generate(&env);

    client.create_contract(&same_party, &same_party, &None, &default_milestones(&env), &ReleaseAuthorization::ClientOnly);
}

#[test]
fn accepts_distinct_client_and_freelancer() {
    let env = Env::default();
    env.mock_all_auths();
    let client = register_client(&env);
    let admin = Address::generate(&env);
    client.initialize(&admin);
    let client_addr = Address::generate(&env);
    let freelancer_addr = Address::generate(&env);

    let id = client.create_contract(&client_addr, &freelancer_addr, &None, &default_milestones(&env), &ReleaseAuthorization::ClientOnly);
    assert_eq!(id, 0);
}

#[test]
#[should_panic(expected = "HostError: Error(Contract, #2)")]
fn rejects_arbiter_equals_client() {
    let env = Env::default();
    env.mock_all_auths();
    let client = register_client(&env);
    let admin = Address::generate(&env);
    client.initialize(&admin);
    let client_addr = Address::generate(&env);
    let freelancer_addr = Address::generate(&env);

    client.create_contract(
        &client_addr,
        &freelancer_addr,
        &Some(client_addr.clone()),
        &default_milestones(&env),
        &ReleaseAuthorization::ClientOnly,
    );
}

#[test]
fn multiple_contracts_with_different_participants() {
    let env = Env::default();
    env.mock_all_auths();
    let client = register_client(&env);
    let admin = Address::generate(&env);
    client.initialize(&admin);

    let alice = Address::generate(&env);
    let bob = Address::generate(&env);
    let charlie = Address::generate(&env);
    let diana = Address::generate(&env);

    let id1 = client.create_contract(&alice, &bob, &None, &default_milestones(&env), &ReleaseAuthorization::ClientOnly);
    assert_eq!(id1, 0);

    let id2 = client.create_contract(
        &charlie,
        &diana,
        &Some(alice.clone()),
        &default_milestones(&env),
        &ReleaseAuthorization::ClientOnly,
    );
    assert_eq!(id2, 1);
}
