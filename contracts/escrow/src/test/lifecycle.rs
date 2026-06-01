use crate::{ContractStatus, DisputeResolution, Escrow, EscrowClient, EscrowError, ReleaseAuthorization};
use soroban_sdk::{testutils::Address as _, testutils::Ledger as _, vec, Address, Env};

fn setup() -> (Env, Address) {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(Escrow, ());
    let client = EscrowClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    client.initialize(&admin);
    (env, contract_id)
}

fn completed_contract(env: &Env, client: &EscrowClient<'_>) -> (Address, Address, u32) {
    let client_addr = Address::generate(env);
    let freelancer_addr = Address::generate(env);
    let id = client.create_contract(
        &client_addr,
        &freelancer_addr,
        &None,
        &vec![env, 100_i128],
        &ReleaseAuthorization::ClientOnly,
    );
    assert!(client.deposit_funds(&id, &client_addr, &100_i128));
    assert!(client.approve_milestone_release(&id, &client_addr, &0));
    assert!(client.release_milestone(&id, &0, &client_addr));
    (client_addr, freelancer_addr, id)
}

fn disputed_contract(env: &Env, client: &EscrowClient<'_>) -> (Address, Address, Address, u32) {
    let client_addr = Address::generate(env);
    let freelancer_addr = Address::generate(env);
    let arbiter = Address::generate(env);
    let id = client.create_contract(
        &client_addr,
        &freelancer_addr,
        &Some(arbiter.clone()),
        &vec![env, 100_i128],
        &ReleaseAuthorization::ClientOnly,
    );
    assert!(client.deposit_funds(&id, &client_addr, &100_i128));
    assert!(client.raise_dispute(&id, &client_addr));
    (client_addr, freelancer_addr, arbiter, id)
}

#[test]
fn finalize_completed_contract_smoke() {
    let (env, contract_addr) = setup();
    let client = EscrowClient::new(&env, &contract_addr);
    let (client_addr_actual, _, id) = completed_contract(&env, &client);

    assert!(client.finalize_contract(&id, &client_addr_actual));
}

#[test]
fn finalized_contract_rejects_subsequent_mutations() {
    let (env, contract_addr) = setup();
    let client = EscrowClient::new(&env, &contract_addr);
    let (client_addr_actual, freelancer_addr, id) = completed_contract(&env, &client);

    assert!(client.finalize_contract(&id, &client_addr_actual));

    super::assert_contract_error(
        client.try_deposit_funds(&id, &client_addr_actual, &1_i128),
        EscrowError::AlreadyFinalized,
    );
    super::assert_contract_error(
        client.try_release_milestone(&id, &0, &client_addr_actual),
        EscrowError::AlreadyFinalized,
    );
    super::assert_contract_error(
        client.try_issue_reputation(&id, &client_addr_actual, &freelancer_addr, &5_i128),
        EscrowError::AlreadyFinalized,
    );
}

#[test]
fn finalized_dispute_rejects_resolution() {
    let (env, contract_addr) = setup();
    let client = EscrowClient::new(&env, &contract_addr);
    let (client_addr_actual, _, arbiter, id) = disputed_contract(&env, &client);

    assert!(client.finalize_contract(&id, &client_addr_actual));

    super::assert_contract_error(
        client.try_resolve_dispute(&id, &arbiter, &DisputeResolution::FullRefund),
        EscrowError::AlreadyFinalized,
    );
}
