use soroban_sdk::{testutils::Address as _, vec, Address, Env};

use crate::{ContractStatus, Escrow, EscrowClient, ReleaseAuthorization};

fn register_client(env: &Env) -> EscrowClient<'_> {
    let id = env.register(Escrow, ());
    EscrowClient::new(env, &id)
}

fn create_default_contract(
    env: &Env,
    client: &EscrowClient,
    client_addr: &Address,
    freelancer_addr: &Address,
) -> u32 {
    let milestones = vec![env, 100_i128, 200_i128, 300_i128];
    client.create_contract(
        client_addr,
        freelancer_addr,
        &None,
        &milestones,
        &ReleaseAuthorization::ClientOnly,
    )
}

#[test]
fn refund_all_unreleased_milestones_and_completes_contract() {
    let env = Env::default();
    env.mock_all_auths();
    let client = register_client(&env);
    let admin = Address::generate(&env);
    client.initialize(&admin);
    let client_addr = Address::generate(&env);
    let freelancer_addr = Address::generate(&env);

    let contract_id = create_default_contract(&env, &client, &client_addr, &freelancer_addr);

    assert!(client.deposit_funds(&contract_id, &client_addr, &1_200_0000000_i128));
    assert!(client.approve_milestone_release(&contract_id, &client_addr, &0));
    assert!(client.release_milestone(&contract_id, &0, &client_addr));

    let refund_ids = vec![&env, 1_u32, 2_u32];
    let refunded = client.refund_unreleased_milestones(&contract_id, &refund_ids);
    assert_eq!(refunded, 1_000_0000000_i128);

    let contract = client.get_contract(&contract_id);
    assert_eq!(contract.status, ContractStatus::Refunded);
    assert_eq!(contract.refunded_amount, 1_000_0000000_i128);
}
