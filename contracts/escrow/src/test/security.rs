use super::{create_default_contract, default_milestones, generated_participants, register_client, setup, create_client};
use crate::{EscrowError, ReleaseAuthorization};
use soroban_sdk::{testutils::Address as _, vec, Env, Vec, Address};

#[test]
fn create_rejects_same_participants() {
    let env = Env::default();
    env.mock_all_auths();
    let client = register_client(&env);
    let (addr, _, _) = generated_participants(&env);

    let result =
        client.try_create_contract(&addr, &addr, &None, &default_milestones(&env), &ReleaseAuthorization::ClientOnly);
    super::assert_contract_error(result, EscrowError::InvalidParticipants);
}

#[test]
fn create_rejects_empty_milestone_list() {
    let env = Env::default();
    env.mock_all_auths();
    let client = register_client(&env);
    let (client_addr, freelancer_addr, _) = generated_participants(&env);
    let empty = Vec::<i128>::new(&env);

    let result =
        client.try_create_contract(&client_addr, &freelancer_addr, &None, &empty, &ReleaseAuthorization::ClientOnly);
    super::assert_contract_error(result, EscrowError::EmptyMilestones);
}

#[test]
fn create_rejects_non_positive_milestone_amount() {
    let env = Env::default();
    env.mock_all_auths();
    let client = register_client(&env);
    let (client_addr, freelancer_addr, _) = generated_participants(&env);
    let milestones = vec![&env, 100_i128, 0_i128];

    let result = client.try_create_contract(
        &client_addr,
        &freelancer_addr,
        &None,
        &milestones,
        &ReleaseAuthorization::ClientOnly,
    );
    super::assert_contract_error(result, EscrowError::InvalidMilestoneAmount);
}

#[test]
fn deposit_rejects_non_positive_amount() {
    let env = Env::default();
    env.mock_all_auths();
    let client = register_client(&env);
    let admin = Address::generate(&env);
    client.initialize(&admin);

    let client_addr = Address::generate(&env);
    let freelancer_addr = Address::generate(&env);
    let id = client.create_contract(&client_addr, &freelancer_addr, &None, &default_milestones(&env), &ReleaseAuthorization::ClientOnly);

    let result = client.try_deposit_funds(&id, &client_addr, &0);
    super::assert_contract_error(result, EscrowError::InvalidDepositAmount);
}

#[test]
fn release_rejects_when_contract_not_funded() {
    let env = Env::default();
    env.mock_all_auths();
    let client = register_client(&env);
    let admin = Address::generate(&env);
    client.initialize(&admin);

    let client_addr = Address::generate(&env);
    let freelancer_addr = Address::generate(&env);
    let id = client.create_contract(&client_addr, &freelancer_addr, &None, &default_milestones(&env), &ReleaseAuthorization::ClientOnly);

    let result = client.try_release_milestone(&id, &0, &client_addr);
    super::assert_contract_error(result, EscrowError::InsufficientFunds);
}

#[test]
fn release_rejects_invalid_milestone_id() {
    let env = Env::default();
    env.mock_all_auths();
    let client = register_client(&env);
    let admin = Address::generate(&env);
    client.initialize(&admin);

    let client_addr = Address::generate(&env);
    let freelancer_addr = Address::generate(&env);
    let id = client.create_contract(&client_addr, &freelancer_addr, &None, &default_milestones(&env), &ReleaseAuthorization::ClientOnly);

    assert!(client.deposit_funds(&id, &client_addr, &super::total_milestone_amount()));
    let result = client.try_release_milestone(&id, &99, &client_addr);
    super::assert_contract_error(result, EscrowError::InvalidMilestone);
}

#[test]
fn release_rejects_double_release() {
    let env = Env::default();
    env.mock_all_auths();
    let client = register_client(&env);
    let admin = Address::generate(&env);
    client.initialize(&admin);

    let client_addr = Address::generate(&env);
    let freelancer_addr = Address::generate(&env);
    let id = client.create_contract(&client_addr, &freelancer_addr, &None, &default_milestones(&env), &ReleaseAuthorization::ClientOnly);

    assert!(client.deposit_funds(&id, &client_addr, &super::total_milestone_amount()));
    assert!(client.approve_milestone_release(&id, &client_addr, &0));
    assert!(client.release_milestone(&id, &0, &client_addr));

    let result = client.try_release_milestone(&id, &0, &client_addr);
    super::assert_contract_error(result, EscrowError::AlreadyReleased);
}

#[test]
fn issue_reputation_rejects_unfinished_contract() {
    let env = Env::default();
    env.mock_all_auths();
    let client = register_client(&env);
    let admin = Address::generate(&env);
    client.initialize(&admin);

    let client_addr = Address::generate(&env);
    let freelancer_addr = Address::generate(&env);
    let id = client.create_contract(&client_addr, &freelancer_addr, &None, &default_milestones(&env), &ReleaseAuthorization::ClientOnly);

    let result = client.try_issue_reputation(&id, &client_addr, &freelancer_addr, &5);
    super::assert_contract_error(result, EscrowError::NotCompleted);
}

#[test]
fn issue_reputation_rejects_unauthorized_caller() {
    let env = Env::default();
    env.mock_all_auths();
    let client = register_client(&env);
    let admin = Address::generate(&env);
    client.initialize(&admin);

    let (c, f, id) = super::complete_contract(&env, &client);
    let unauthorized = soroban_sdk::Address::generate(&env);

    let result = client.try_issue_reputation(&id, &unauthorized, &f, &5);
    super::assert_contract_error(result, EscrowError::UnauthorizedRole);
}
