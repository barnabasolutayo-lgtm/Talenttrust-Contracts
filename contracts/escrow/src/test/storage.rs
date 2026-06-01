use super::{
    assert_contract_error, complete_contract, create_default_contract, default_milestones,
    generated_participants, register_client, total_milestone_amount, MILESTONE_ONE, MILESTONE_TWO,
    setup, create_client
};
use crate::{ContractStatus, DataKey, EscrowError, ReadinessChecklist};
use soroban_sdk::{testutils::Address as _, Address, Env, Symbol};

// ─── Initialized / Admin ──────────────────────────────────────────────────────

#[test]
fn initialized_written_on_initialize() {
    let env = Env::default();
    env.mock_all_auths();
    let client = register_client(&env);
    let admin = Address::generate(&env);

    assert!(client.initialize(&admin));

    env.as_contract(&client.address, || {
        let v: bool = env
            .storage()
            .persistent()
            .get(&DataKey::Initialized)
            .unwrap();
        assert!(v);
    });
}

#[test]
fn admin_written_on_initialize() {
    let env = Env::default();
    env.mock_all_auths();
    let client = register_client(&env);
    let admin = Address::generate(&env);

    client.initialize(&admin);

    env.as_contract(&client.address, || {
        let stored: Address = env.storage().persistent().get(&DataKey::Admin).unwrap();
        assert_eq!(stored, admin);
    });
}

#[test]
fn double_initialize_fails() {
    let env = Env::default();
    env.mock_all_auths();
    let client = register_client(&env);
    let admin = Address::generate(&env);

    client.initialize(&admin);
    assert_contract_error(
        client.try_initialize(&admin),
        EscrowError::AlreadyInitialized,
    );
}

// ─── Paused ───────────────────────────────────────────────────────────────────

#[test]
fn paused_written_by_pause_and_cleared_by_unpause() {
    let env = Env::default();
    env.mock_all_auths();
    let client = register_client(&env);
    let admin = Address::generate(&env);
    client.initialize(&admin);

    client.pause();
    env.as_contract(&client.address, || {
        let v: bool = env
            .storage()
            .persistent()
            .has(&Symbol::new(&env, "paused"));
        assert!(v);
    });

    client.unpause();
    env.as_contract(&client.address, || {
        let v: bool = env
            .storage()
            .persistent()
            .has(&Symbol::new(&env, "paused"));
        assert!(!v);
    });
}

#[test]
fn paused_blocks_create_contract() {
    let env = Env::default();
    env.mock_all_auths();
    let client = register_client(&env);
    let admin = Address::generate(&env);
    client.initialize(&admin);
    client.pause();

    let (c, f, _) = generated_participants(&env);
    assert_contract_error(
        client.try_create_contract(
            &c,
            &f,
            &None,
            &default_milestones(&env),
            &crate::types::ReleaseAuthorization::ClientOnly,
        ),
        EscrowError::ContractPaused,
    );
}

#[test]
fn paused_blocks_deposit_funds() {
    let env = Env::default();
    env.mock_all_auths();
    let client = register_client(&env);
    let admin = Address::generate(&env);
    client.initialize(&admin);

    let client_addr = Address::generate(&env);
    let freelancer_addr = Address::generate(&env);
    let id = client.create_contract(&client_addr, &freelancer_addr, &None, &default_milestones(&env), &crate::types::ReleaseAuthorization::ClientOnly);
    client.pause();

    assert_contract_error(
        client.try_deposit_funds(&id, &client_addr, &total_milestone_amount()),
        EscrowError::ContractPaused,
    );
}

#[test]
fn paused_blocks_release_milestone() {
    let env = Env::default();
    env.mock_all_auths();
    let client = register_client(&env);
    let admin = Address::generate(&env);
    client.initialize(&admin);

    let client_addr = Address::generate(&env);
    let freelancer_addr = Address::generate(&env);
    let id = client.create_contract(&client_addr, &freelancer_addr, &None, &default_milestones(&env), &crate::types::ReleaseAuthorization::ClientOnly);
    client.deposit_funds(&id, &client_addr, &total_milestone_amount());
    client.pause();

    assert_contract_error(
        client.try_release_milestone(&id, &0, &client_addr),
        EscrowError::ContractPaused,
    );
}

#[test]
fn paused_blocks_cancel_contract() {
    let env = Env::default();
    env.mock_all_auths();
    let client = register_client(&env);
    let admin = Address::generate(&env);
    client.initialize(&admin);

    let client_addr = Address::generate(&env);
    let freelancer_addr = Address::generate(&env);
    let id = client.create_contract(&client_addr, &freelancer_addr, &None, &default_milestones(&env), &crate::types::ReleaseAuthorization::ClientOnly);
    client.pause();

    assert_contract_error(
        client.try_cancel_contract(&id, &client_addr),
        EscrowError::ContractPaused,
    );
}

#[test]
fn read_only_queries_not_blocked_by_pause() {
    let env = Env::default();
    env.mock_all_auths();
    let client = register_client(&env);
    let admin = Address::generate(&env);
    client.initialize(&admin);

    let client_addr = Address::generate(&env);
    let freelancer_addr = Address::generate(&env);
    let id = client.create_contract(&client_addr, &freelancer_addr, &None, &default_milestones(&env), &crate::types::ReleaseAuthorization::ClientOnly);
    client.pause();

    let record = client.get_contract(&id);
    assert_eq!(record.status, ContractStatus::Created);
    assert!(client.is_paused());
}

// ─── Emergency ────────────────────────────────────────────────────────────────

#[test]
fn emergency_written_by_activate_and_cleared_by_resolve() {
    let env = Env::default();
    env.mock_all_auths();
    let client = register_client(&env);
    let admin = Address::generate(&env);
    client.initialize(&admin);

    client.activate_emergency_pause();
    env.as_contract(&client.address, || {
        let v: bool = env
            .storage()
            .persistent()
            .has(&Symbol::new(&env, "emergency"));
        assert!(v);
    });

    client.resolve_emergency();
    env.as_contract(&client.address, || {
        let v: bool = env
            .storage()
            .persistent()
            .has(&Symbol::new(&env, "emergency"));
        assert!(!v);
    });
}

// ─── Contract / NextContractId ────────────────────────────────────────────────

#[test]
fn contract_written_on_create_and_readable() {
    let env = Env::default();
    env.mock_all_auths();
    let client = register_client(&env);
    let (c, f, _) = generated_participants(&env);

    let id = client.create_contract(
        &c,
        &f,
        &None,
        &default_milestones(&env),
        &crate::types::ReleaseAuthorization::ClientOnly,
    );

    let record = client.get_contract(&id);
    assert_eq!(record.client, c);
    assert_eq!(record.freelancer, f);
    assert_eq!(record.status, ContractStatus::Created);
    assert_eq!(record.total_deposited, 0);
}

#[test]
fn next_contract_id_increments_per_contract() {
    let env = Env::default();
    env.mock_all_auths();
    let client = register_client(&env);

    let client_addr = Address::generate(&env);
    let freelancer_addr = Address::generate(&env);
    let id1 = client.create_contract(&client_addr, &freelancer_addr, &None, &default_milestones(&env), &crate::types::ReleaseAuthorization::ClientOnly);
    let id2 = client.create_contract(&client_addr, &freelancer_addr, &None, &default_milestones(&env), &crate::types::ReleaseAuthorization::ClientOnly);
    assert_eq!(id2, id1 + 1);
}

// ─── MilestoneReleased ────────────────────────────────────────────────────────

#[test]
fn milestone_status_updated_on_release() {
    let env = Env::default();
    env.mock_all_auths();
    let client = register_client(&env);

    let client_addr = Address::generate(&env);
    let freelancer_addr = Address::generate(&env);
    let id = client.create_contract(&client_addr, &freelancer_addr, &None, &default_milestones(&env), &crate::types::ReleaseAuthorization::ClientOnly);
    
    client.deposit_funds(&id, &client_addr, &total_milestone_amount());
    client.approve_milestone_release(&id, &client_addr, &0);
    client.release_milestone(&id, &0, &client_addr);

    let milestones = client.get_milestones(&id);
    assert!(milestones.get(0).unwrap().released);
    assert!(!milestones.get(1).unwrap().released);
}

// ─── Reputation ───────────────────────────────────────────────────────────────

#[test]
fn issue_reputation_works() {
    let env = Env::default();
    env.mock_all_auths();
    let client = register_client(&env);

    let (c, f, id) = complete_contract(&env, &client);
    // client.issue_reputation is already updated to match lib.rs which has true return
    assert!(client.issue_reputation(&id, &c, &f, &5));
}

// ─── Accounting invariant ─────────────────────────────────────────────────────

#[test]
fn released_amount_tracks_milestone_amounts() {
    let env = Env::default();
    env.mock_all_auths();
    let client = register_client(&env);

    let client_addr = Address::generate(&env);
    let freelancer_addr = Address::generate(&env);
    let id = client.create_contract(&client_addr, &freelancer_addr, &None, &default_milestones(&env), &crate::types::ReleaseAuthorization::ClientOnly);
    client.deposit_funds(&id, &client_addr, &total_milestone_amount());

    client.approve_milestone_release(&id, &client_addr, &0);
    client.release_milestone(&id, &0, &client_addr);
    let r = client.get_contract(&id);
    assert_eq!(r.released_amount, MILESTONE_ONE);

    client.approve_milestone_release(&id, &client_addr, &1);
    client.release_milestone(&id, &1, &client_addr);
    let r = client.get_contract(&id);
    assert_eq!(r.released_amount, MILESTONE_ONE + MILESTONE_TWO);
}
