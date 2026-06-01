use super::{create_default_contract, register_client, total_milestone_amount, create_client, setup};
use soroban_sdk::{Env, Address, testutils::Address as _};
use crate::ReleaseAuthorization;

#[test]
fn performance_smoke_test() {
    let env = Env::default();
    env.mock_all_auths();
    let client = register_client(&env);
    let admin = Address::generate(&env);
    client.initialize(&admin);

    let client_addr = Address::generate(&env);
    let freelancer_addr = Address::generate(&env);
    let id = client.create_contract(&client_addr, &freelancer_addr, &None, &super::default_milestones(&env), &ReleaseAuthorization::ClientOnly);

    client.deposit_funds(&id, &client_addr, &total_milestone_amount());
    
    assert!(client.approve_milestone_release(&id, &client_addr, &0));
    client.release_milestone(&id, &0, &client_addr);
}
