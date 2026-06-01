#![cfg(test)]

use soroban_sdk::{
    testutils::Address as _,
    vec, Address, Env, Vec,
};

use crate::{
    ContractStatus, Escrow, EscrowClient, MilestoneSchedule, ReleaseAuthorization,
};

fn register_client(env: &Env) -> EscrowClient<'_> {
    let contract_id = env.register(Escrow, ());
    EscrowClient::new(env, &contract_id)
}

fn setup_funded_contract(
    env: &Env,
    arbiter: Option<Address>,
) -> (EscrowClient<'_>, Address, Address, Option<Address>, u32, u64) {
    env.mock_all_auths();

    let client = register_client(env);
    let admin = Address::generate(env);
    client.initialize(&admin);

    let client_addr = Address::generate(env);
    let freelancer_addr = Address::generate(env);
    let due_date = env.ledger().timestamp() + 100;

    let contract_id = client.create_contract(
        &client_addr,
        &freelancer_addr,
        &arbiter,
        &vec![env, 1_0000000_i128],
        &ReleaseAuthorization::ClientOnly,
    );
    
    // Set schedule if we want to test timeout
    client.set_milestone_schedule(&contract_id, &0, &MilestoneSchedule {
        due_date: Some(due_date),
        title: None,
        description: None,
        updated_at: 0,
    });

    assert!(client.deposit_funds(&contract_id, &client_addr, &1_0000000_i128));

    (
        client,
        client_addr,
        freelancer_addr,
        arbiter,
        contract_id,
        due_date,
    )
}

#[test]
fn approval_is_allowed_at_exact_deadline() {
    let env = Env::default();
    let (client, client_addr, _, _, contract_id, due_date) = setup_funded_contract(&env, None);

    env.ledger().with_mut(|li| {
        li.timestamp = due_date;
    });

    assert!(client.approve_milestone_release(&contract_id, &client_addr, &0));
}

#[test]
#[should_panic]
fn approval_past_deadline_is_rejected() {
    let env = Env::default();
    let (client, client_addr, _, _, contract_id, due_date) = setup_funded_contract(&env, None);

    env.ledger().with_mut(|li| {
        li.timestamp = due_date + 1;
    });

    client.approve_milestone_release(&contract_id, &client_addr, &0);
}

#[test]
fn evaluate_timeout_smoke() {
    let env = Env::default();
    let (client, _, _, _, contract_id, due_date) = setup_funded_contract(&env, None);

    env.ledger().with_mut(|li| {
        li.timestamp = due_date + 1;
    });

    assert!(client.evaluate_milestone_timeout(&contract_id, &0));
}

#[test]
#[should_panic]
fn release_past_deadline_is_rejected() {
    let env = Env::default();
    let (client, client_addr, _, _, contract_id, due_date) = setup_funded_contract(&env, None);

    env.ledger().with_mut(|li| {
        li.timestamp = due_date;
    });
    assert!(client.approve_milestone_release(&contract_id, &client_addr, &0));

    env.ledger().with_mut(|li| {
        li.timestamp = due_date + 1;
    });
    client.release_milestone(&contract_id, &0, &client_addr);
}

#[test]
fn arbiter_resolves_timeout_dispute_smoke() {
    let env = Env::default();
    let arbiter = Address::generate(&env);
    let (client, _client_addr, _, _, contract_id, due_date) =
        setup_funded_contract(&env, Some(arbiter.clone()));

    env.ledger().with_mut(|li| {
        li.timestamp = due_date + 1;
    });
    assert!(client.evaluate_milestone_timeout(&contract_id, &0));

    assert!(client.resolve_dispute_simple(&contract_id, &arbiter));
}
