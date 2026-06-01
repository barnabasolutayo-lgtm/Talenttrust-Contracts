use super::{complete_contract, create_default_contract, register_client, total_milestone_amount, setup, create_client};
use crate::{ContractStatus, EscrowError, ReleaseAuthorization};
use soroban_sdk::{testutils::Address as _, vec, Address, Env};

#[test]
fn contract_state_round_trips_across_lifecycle_mutations() {
    let env = Env::default();
    env.mock_all_auths();
    let client = register_client(&env);
    let admin = Address::generate(&env);
    client.initialize(&admin);

    let client_addr = Address::generate(&env);
    let freelancer_addr = Address::generate(&env);
    let id = client.create_contract(&client_addr, &freelancer_addr, &None, &super::default_milestones(&env), &ReleaseAuthorization::ClientOnly);
    
    let created = client.get_contract(&id);
    assert_eq!(created.client, client_addr);
    assert_eq!(created.freelancer, freelancer_addr);
    assert_eq!(created.status, ContractStatus::Created);

    assert!(client.deposit_funds(&id, &client_addr, &total_milestone_amount()));
    let funded = client.get_contract(&id);
    assert_eq!(funded.status, ContractStatus::Funded);

    assert!(client.approve_milestone_release(&id, &client_addr, &0));
    assert!(client.release_milestone(&id, &0, &client_addr));

    let after_release = client.get_contract(&id);
    assert_eq!(after_release.released_amount, super::MILESTONE_ONE);
}

#[test]
fn participant_metadata_and_pending_credits_persist() {
    let env = Env::default();
    env.mock_all_auths();
    let client = register_client(&env);
    let admin = Address::generate(&env);
    client.initialize(&admin);

    let (client_addr, freelancer_addr, id) = complete_contract(&env, &client);
    let completed = client.get_contract(&id);
    assert_eq!(completed.client, client_addr);
    assert_eq!(completed.freelancer, freelancer_addr);

    assert!(client.issue_reputation(&id, &client_addr, &freelancer_addr, &5));
}

#[test]
fn try_get_contract_reports_missing_state() {
    let env = Env::default();
    env.mock_all_auths();
    let client = register_client(&env);
    let admin = Address::generate(&env);
    client.initialize(&admin);

    super::assert_contract_error(client.try_get_contract(&777), EscrowError::ContractNotFound);
}
