use crate::{ContractStatus, Escrow, EscrowClient, EscrowError, ReleaseAuthorization};
use soroban_sdk::{symbol_short, testutils::Address as _, vec, Address, Env};

fn setup_initialized() -> (Env, Address, Address) {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(Escrow, ());
    let client = EscrowClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    assert!(client.initialize(&admin));
    (env, contract_id, admin)
}

/// Create a funded contract with one milestone ready to release.
fn setup_funded_contract(env: &Env, client: &EscrowClient) -> (Address, Address, u32) {
    let client_addr = Address::generate(env);
    let freelancer_addr = Address::generate(env);
    let milestones = vec![env, 100_i128, 200_i128];
    let id = client.create_contract(
        &client_addr,
        &freelancer_addr,
        &None,
        &milestones,
        &ReleaseAuthorization::ClientOnly,
    );
    client.deposit_funds(&id, &client_addr, &300_i128);
    (client_addr, freelancer_addr, id)
}

/// Create a completed contract ready for reputation issuance.
fn setup_completed_contract(env: &Env, client: &EscrowClient) -> (Address, Address, u32) {
    let (client_addr, freelancer_addr, id) = setup_funded_contract(env, client);
    client.approve_milestone_release(&id, &client_addr, &0);
    client.release_milestone(&id, &client_addr, &0);
    client.approve_milestone_release(&id, &client_addr, &1);
    client.release_milestone(&id, &client_addr, &0);
    client.release_milestone(&id, &client_addr, &1);
    (client_addr, freelancer_addr, id)
}

// ─── initialize ──────────────────────────────────────────────────────────────

#[test]
fn initialize_only_once_fails() {
    let (env, contract_id, admin) = setup_initialized();
    let client = EscrowClient::new(&env, &contract_id);
    super::assert_contract_error(
        client.try_initialize(&admin),
        EscrowError::AlreadyInitialized,
    );
}

// ─── pause / unpause ─────────────────────────────────────────────────────────

#[test]
fn pause_then_unpause_toggles_state() {
    let (env, contract_id, _admin) = setup_initialized();
    let client = EscrowClient::new(&env, &contract_id);

    assert!(!client.is_paused());
    assert!(client.pause());
    assert!(client.is_paused());
    assert!(client.unpause());
    assert!(!client.is_paused());
}

#[test]
fn pause_requires_initialization() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(Escrow, ());
    let client = EscrowClient::new(&env, &contract_id);
    super::assert_contract_error(client.try_pause(), EscrowError::NotInitialized);
}

// ─── create_contract blocked ─────────────────────────────────────────────────

#[test]
fn pause_blocks_create_contract() {
    let (env, contract_id, _admin) = setup_initialized();
    let client = EscrowClient::new(&env, &contract_id);
    client.pause();

    let a = Address::generate(&env);
    let b = Address::generate(&env);
    super::assert_contract_error(
        client.try_create_contract(
            &a,
            &b,
            &None,
            &vec![&env, 50_i128],
            &ReleaseAuthorization::ClientOnly,
        ),
        EscrowError::ContractPaused,
    );
}

// ─── deposit_funds blocked ───────────────────────────────────────────────────

#[test]
fn pause_blocks_deposit_funds() {
    let (env, contract_id, _admin) = setup_initialized();
    let client = EscrowClient::new(&env, &contract_id);
    let (client_addr, _, id) = setup_funded_contract(&env, &client);
    client.pause();

    let caller = Address::generate(&env);
    super::assert_contract_error(
        client.try_deposit_funds(&id, &caller, &50_i128),
        client.try_deposit_funds(&id, &client_addr, &50_i128),
        EscrowError::ContractPaused,
    );
}

// ─── release_milestone blocked ───────────────────────────────────────────────

#[test]
fn pause_blocks_release_milestone() {
    let (env, contract_id, _admin) = setup_initialized();
    let client = EscrowClient::new(&env, &contract_id);
    let (client_addr, _, id) = setup_funded_contract(&env, &client);
    client.pause();

    let caller = Address::generate(&env);
    super::assert_contract_error(
        client.try_release_milestone(&id, &caller, &0),
        client.try_release_milestone(&id, &client_addr, &0),
        EscrowError::ContractPaused,
    );
}

// ─── issue_reputation blocked ────────────────────────────────────────────────

#[test]
fn pause_blocks_issue_reputation() {
    let (env, contract_id, _admin) = setup_initialized();
    let client = EscrowClient::new(&env, &contract_id);
    let (client_addr, freelancer_addr, id) = setup_completed_contract(&env, &client);
    client.pause();

    super::assert_contract_error(
        client.try_issue_reputation(&id, &client_addr, &freelancer_addr, &5_i128),
        EscrowError::ContractPaused,
    );
}

// ─── cancel_contract blocked ─────────────────────────────────────────────────

#[test]
fn pause_blocks_cancel_contract() {
    let (env, contract_id, _admin) = setup_initialized();
    let client = EscrowClient::new(&env, &contract_id);
    let (client_addr, _, id) = setup_funded_contract(&env, &client);
    client.pause();

    super::assert_contract_error(
        client.try_cancel_contract(&id, &client_addr),
        EscrowError::ContractPaused,
    );
}

// ─── unpaused allows operations ──────────────────────────────────────────────

#[test]
fn unpause_restores_create_contract() {
    let (env, contract_id, _admin) = setup_initialized();
    let client = EscrowClient::new(&env, &contract_id);
    client.pause();
    client.unpause();

    let a = Address::generate(&env);
    let b = Address::generate(&env);
    let id = client.create_contract(
        &a,
        &b,
        &None,
        &vec![&env, 50_i128],
        &ReleaseAuthorization::ClientOnly,
    );
    assert_eq!(id, 1);
}

// ─── cancelled event emission ──────────────────────────────────────────────────

/// cancelled event is emitted on successful cancellation with correct payload.
/// Validates event topic and payload structure for indexer observability.
#[test]
fn cancel_contract_emits_cancelled_event() {
    let env = Env::default();
    env.mock_all_auths();
    let client = EscrowClient::new(&env, &env.register(Escrow, ()));
    let admin = Address::generate(&env);
    assert!(client.initialize(&admin));

    let client_addr = Address::generate(&env);
    let freelancer_addr = Address::generate(&env);
    let milestones = vec![&env, 100_i128, 200_i128, 300_i128];
    let contract_id = client.create_contract(
        &client_addr,
        &freelancer_addr,
        &None,
        &milestones,
        &ReleaseAuthorization::ClientOnly,
    );

    // Capture timestamp before cancellation
    let expected_timestamp = env.ledger().timestamp();

    // Cancel the contract
    assert!(client.cancel_contract(&contract_id, &client_addr));

    // Verify the cancelled event was emitted
    let events = env.events().all();
    let cancelled_event = events
        .iter()
        .find(|(topic, _)| topic == &(symbol_short!("cancelled"), contract_id));

    assert!(cancelled_event.is_some(), "cancelled event must be emitted");

    // Verify payload: (caller, previous_status, timestamp)
    let (_, payload) = cancelled_event.unwrap();
    assert_eq!(payload.get(0).unwrap(), client_addr); // caller
    assert_eq!(payload.get(1).unwrap(), ContractStatus::Created); // previous_status
    assert_eq!(payload.get(2).unwrap(), expected_timestamp); // timestamp
}

/// cancelled event contains correct previous_status for Funded state.
/// Validates that the prior state is captured before transition.
#[test]
fn cancel_contract_emits_event_with_previous_status_funded() {
    let env = Env::default();
    env.mock_all_auths();
    let client = EscrowClient::new(&env, &env.register(Escrow, ()));
    let admin = Address::generate(&env);
    assert!(client.initialize(&admin));

    let client_addr = Address::generate(&env);
    let freelancer_addr = Address::generate(&env);
    let milestones = vec![&env, 100_i128, 200_i128, 300_i128];
    let contract_id = client.create_contract(
        &client_addr,
        &freelancer_addr,
        &None,
        &milestones,
        &ReleaseAuthorization::ClientOnly,
    );

    // Fund the contract (transitions to Funded state)
    client.deposit_funds(&contract_id, &client_addr, &600_i128);

    // Cancel as freelancer
    let expected_timestamp = env.ledger().timestamp();
    assert!(client.cancel_contract(&contract_id, &freelancer_addr));

    // Verify the cancelled event was emitted with Funded as previous_status
    let events = env.events().all();
    let cancelled_event = events
        .iter()
        .find(|(topic, _)| topic == &(symbol_short!("cancelled"), contract_id));

    assert!(cancelled_event.is_some());
    let (_, payload) = cancelled_event.unwrap();
    assert_eq!(payload.get(0).unwrap(), freelancer_addr); // caller
    assert_eq!(payload.get(1).unwrap(), ContractStatus::Funded); // previous_status
    assert_eq!(payload.get(2).unwrap(), expected_timestamp); // timestamp
}

/// cancelled event is not emitted on failed cancellation.
/// Validates security invariant: event only on successful state transition.
#[test]
fn cancel_contract_no_event_on_invalid_state() {
    let env = Env::default();
    env.mock_all_auths();
    let client = EscrowClient::new(&env, &env.register(Escrow, ()));
    let admin = Address::generate(&env);
    assert!(client.initialize(&admin));

    let client_addr = Address::generate(&env);
    let freelancer_addr = Address::generate(&env);
    let milestones = vec![&env, 100_i128, 200_i128, 300_i128];
    let contract_id = client.create_contract(
        &client_addr,
        &freelancer_addr,
        &None,
        &milestones,
        &ReleaseAuthorization::ClientOnly,
    );

    // Cancel once - event emitted
    assert!(client.cancel_contract(&contract_id, &client_addr));

    // Attempt second cancellation - should fail, no additional event
    let result = client.try_cancel_contract(&contract_id, &client_addr);
    assert!(result.is_err(), "Second cancellation should fail");

    // Only one cancelled event should exist
    let events = env.events().all();
    let cancelled_events: Vec<_> = events
        .iter()
        .filter(|(topic, _)| topic == &(symbol_short!("cancelled"), contract_id))
        .collect();
    assert_eq!(cancelled_events.len(), 1, "Only one cancelled event should be emitted");
}
