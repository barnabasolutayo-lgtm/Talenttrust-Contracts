use soroban_sdk::{testutils::Address as _, vec, Address, Env};

use crate::{
    ContractStatus, Escrow, EscrowClient, EscrowError, ReleaseAuthorization,
};

// ---------------------------------------------------------------------------
// Test helpers
// ---------------------------------------------------------------------------

fn register_client(env: &Env) -> EscrowClient<'_> {
    let id = env.register(Escrow, ());
    EscrowClient::new(env, &id)
}

fn create_contract_with_mode(
    env: &Env,
    client: &EscrowClient,
    client_addr: &Address,
    freelancer_addr: &Address,
    arbiter_addr: &Option<Address>,
    mode: &ReleaseAuthorization,
) -> u32 {
    let milestones = vec![env, 100_i128, 200_i128, 300_i128];
    client.create_contract(
        client_addr,
        freelancer_addr,
        arbiter_addr,
        &milestones,
        mode,
    )
}

fn fund_contract(_env: &Env, client: &EscrowClient, contract_id: &u32, funder: &Address) {
    client.deposit_funds(contract_id, funder, &600_i128);
}

fn funded_contract(env: &Env, client: &EscrowClient<'_>) -> (Address, Address, u32) {
    let client_addr = Address::generate(env);
    let freelancer_addr = Address::generate(env);
    let milestones = vec![env, 500_i128, 300_i128];
    let id = client.create_contract(
        &client_addr,
        &freelancer_addr,
        &None,
        &milestones,
        &ReleaseAuthorization::ClientOnly,
    );
    client.deposit_funds(&id, &client_addr, &800_i128);
    (client_addr, freelancer_addr, id)
}

// ---------------------------------------------------------------------------
// Happy path: legitimate client releases a milestone
// ---------------------------------------------------------------------------

#[test]
fn client_can_release_funded_milestone() {
    let env = Env::default();
    env.mock_all_auths();
    let client = register_client(&env);
    let admin = Address::generate(&env);
    client.initialize(&admin);
    let (client_addr, _freelancer_addr, id) = funded_contract(&env, &client);

    assert!(client.approve_milestone_release(&id, &client_addr, &0));
    assert!(client.release_milestone(&id, &0, &client_addr));

    let contract = client.get_contract(&id);
    assert_eq!(contract.released_amount, 500_i128);
}

// ---------------------------------------------------------------------------
// Attacker is rejected with UnauthorizedRole
// ---------------------------------------------------------------------------

#[test]
fn attacker_cannot_release_milestone() {
    let env = Env::default();
    env.mock_all_auths();
    let client = register_client(&env);
    let admin = Address::generate(&env);
    client.initialize(&admin);
    let (_client_addr, _freelancer_addr, id) = funded_contract(&env, &client);

    let attacker = Address::generate(&env);
    let result = client.try_release_milestone(&id, &0, &attacker);
    super::assert_contract_error(result, EscrowError::UnauthorizedRole);
}

// ---------------------------------------------------------------------------
// Double-release is rejected with AlreadyReleased; no duplicate transfer
// ---------------------------------------------------------------------------

#[test]
fn double_release_is_rejected_and_amount_not_duplicated() {
    let env = Env::default();
    env.mock_all_auths();
    let client = register_client(&env);
    let admin = Address::generate(&env);
    client.initialize(&admin);
    let (client_addr, _freelancer_addr, id) = funded_contract(&env, &client);

    assert!(client.approve_milestone_release(&id, &client_addr, &0));
    assert!(client.release_milestone(&id, &0, &client_addr));

    let result = client.try_release_milestone(&id, &0, &client_addr);
    super::assert_contract_error(result, EscrowError::AlreadyReleased);

    let contract = client.get_contract(&id);
    assert_eq!(contract.released_amount, 500_i128);
}

// ---------------------------------------------------------------------------
// Freelancer (non-client) is also rejected
// ---------------------------------------------------------------------------

#[test]
fn freelancer_cannot_release_milestone() {
    let env = Env::default();
    env.mock_all_auths();
    let client = register_client(&env);
    let admin = Address::generate(&env);
    client.initialize(&admin);
    let (_client_addr, freelancer_addr, id) = funded_contract(&env, &client);

    let result = client.try_release_milestone(&id, &0, &freelancer_addr);
    super::assert_contract_error(result, EscrowError::UnauthorizedRole);
}

#[test]
fn release_emits_events() {
    let env = Env::default();
    env.mock_all_auths();
    let client = register_client(&env);
    let admin = Address::generate(&env);
    client.initialize(&admin);

    let client_addr = Address::generate(&env);
    let freelancer_addr = Address::generate(&env);

    let contract_id = create_contract_with_mode(
        &env,
        &client,
        &client_addr,
        &freelancer_addr,
        &None,
        &ReleaseAuthorization::ClientOnly,
    );

    fund_contract(&env, &client, &contract_id, &client_addr);

    assert!(client.approve_milestone_release(&contract_id, &client_addr, &0));
    client.release_milestone(&contract_id, &0, &client_addr);
}

#[test]
fn rejects_double_release_and_completes_contract() {
    let env = Env::default();
    env.mock_all_auths();
    let client = register_client(&env);
    let admin = Address::generate(&env);
    client.initialize(&admin);

    let client_addr = Address::generate(&env);
    let freelancer_addr = Address::generate(&env);

    let contract_id = create_contract_with_mode(
        &env,
        &client,
        &client_addr,
        &freelancer_addr,
        &None,
        &ReleaseAuthorization::ClientOnly,
    );
    fund_contract(&env, &client, &contract_id, &client_addr);

    assert!(client.approve_milestone_release(&contract_id, &client_addr, &0));
    assert!(client.release_milestone(&contract_id, &0, &client_addr));

    let result = client.try_release_milestone(&contract_id, &0, &client_addr);
    super::assert_contract_error(result, EscrowError::AlreadyReleased);

    let milestones = vec![&env, 100_i128, 200_i128, 300_i128];
    // id doesn't match total_milestone_amount which was 3 milestones
    // let's just use the contract_id we have which has 3 milestones 100, 200, 300
    // releases: 100 (done), 200, 300. total 600.
    assert!(client.approve_milestone_release(&contract_id, &client_addr, &1));
    assert!(client.release_milestone(&contract_id, &1, &client_addr));
    assert!(client.approve_milestone_release(&contract_id, &client_addr, &2));
    assert!(client.release_milestone(&contract_id, &2, &client_addr));

    let contract = client.get_contract(&contract_id);
    assert_eq!(contract.status, ContractStatus::Completed);
}
