#![cfg(test)]

use soroban_sdk::{testutils::Address as _, vec, Address, Env, String, Vec};

use crate::{
    Escrow, EscrowClient, MilestoneSchedule, ReleaseAuthorization,
};

fn register_client(env: &Env) -> EscrowClient<'_> {
    let id = env.register(Escrow, ());
    EscrowClient::new(env, &id)
}

fn participants(env: &Env) -> (Address, Address) {
    (Address::generate(env), Address::generate(env))
}

fn two_milestones(env: &Env) -> Vec<i128> {
    vec![env, 100_i128, 200_i128]
}

fn three_milestones(env: &Env) -> Vec<i128> {
    vec![env, 100_i128, 200_i128, 300_i128]
}

fn future(env: &Env, offset_secs: u64) -> u64 {
    env.ledger().timestamp() + offset_secs
}

fn dated_schedule(_env: &Env, due: u64) -> MilestoneSchedule {
    MilestoneSchedule {
        due_date: Some(due),
        title: None,
        description: None,
        updated_at: 0,
    }
}

fn full_schedule(env: &Env, due: u64, title: &str, desc: &str) -> MilestoneSchedule {
    MilestoneSchedule {
        due_date: Some(due),
        title: Some(String::from_str(env, title)),
        description: Some(String::from_str(env, desc)),
        updated_at: 0,
    }
}

#[test]
fn valid_create_without_schedules() {
    let env = Env::default();
    env.mock_all_auths();
    let client = register_client(&env);
    let admin = Address::generate(&env);
    client.initialize(&admin);
    let (c, f) = participants(&env);

    let id = client.create_contract(
        &c,
        &f,
        &None,
        &two_milestones(&env),
        &ReleaseAuthorization::ClientOnly,
    );

    assert!(client.get_milestone_schedule(&id, &0).is_none());
}

#[test]
fn valid_create_with_schedules_via_set() {
    let env = Env::default();
    env.mock_all_auths();
    let client = register_client(&env);
    let admin = Address::generate(&env);
    client.initialize(&admin);
    let (c, f) = participants(&env);

    let id = client.create_contract(
        &c,
        &f,
        &None,
        &two_milestones(&env),
        &ReleaseAuthorization::ClientOnly,
    );

    let due = future(&env, 86_400);
    let sched = dated_schedule(&env, due);
    assert!(client.set_milestone_schedule(&id, &0, &sched));

    let stored = client.get_milestone_schedule(&id, &0).expect("schedule should exist");
    assert_eq!(stored.due_date, Some(due));
}

#[test]
fn set_schedule_client_can_update_before_release() {
    let env = Env::default();
    env.mock_all_auths();
    let client = register_client(&env);
    let admin = Address::generate(&env);
    client.initialize(&admin);
    let (c, f) = participants(&env);

    let id = client.create_contract(
        &c,
        &f,
        &None,
        &vec![&env, 100_i128],
        &ReleaseAuthorization::ClientOnly,
    );

    let new_due = future(&env, 50_000);
    let new_sched = full_schedule(&env, new_due, "Updated title", "Updated desc");

    assert!(client.set_milestone_schedule(&id, &0, &new_sched));

    let stored = client.get_milestone_schedule(&id, &0).expect("should exist after set");
    assert_eq!(stored.due_date, Some(new_due));
}

#[test]
fn integration_full_lifecycle_preserves_schedule_metadata() {
    let env = Env::default();
    env.mock_all_auths();
    let client = register_client(&env);
    let admin = Address::generate(&env);
    client.initialize(&admin);
    let (c, f) = participants(&env);

    let id = client.create_contract(
        &c,
        &f,
        &None,
        &two_milestones(&env),
        &ReleaseAuthorization::ClientOnly,
    );

    let due0 = future(&env, 100_000);
    client.set_milestone_schedule(&id, &0, &dated_schedule(&env, due0));

    client.deposit_funds(&id, &c, &300_i128);
    client.approve_milestone_release(&id, &c, &0);
    client.release_milestone(&id, &0, &c);

    let s0 = client.get_milestone_schedule(&id, &0).unwrap();
    assert_eq!(s0.due_date, Some(due0));
}
