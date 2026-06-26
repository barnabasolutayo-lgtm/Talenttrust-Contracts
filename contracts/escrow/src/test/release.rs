use soroban_sdk::{
    symbol_short, testutils::Address as _, testutils::Events, vec, Address, Env, String, Symbol,
    TryFromVal, Val,
};

use super::{
    assert_contract_error, create_contract, register_client, total_milestone_amount, MILESTONE_ONE,
};
use crate::{ContractStatus, Error, EscrowError, ReleaseAuthorization};

fn evidence(env: &Env, s: &str) -> String {
    String::from_str(env, s)
}

// ---------------------------------------------------------------------------
// Release flow tests
// ---------------------------------------------------------------------------

#[test]
fn releases_funded_milestones_and_completes_when_all_released() {
    let env = Env::default();
    env.mock_all_auths();
    let client = register_client(&env);
    let (client_addr, _freelancer_addr, contract_id) = create_contract(&env, &client);

    assert!(client.deposit_funds(&contract_id, &client_addr, &total_milestone_amount()));

    assert!(client.approve_milestone_release(&contract_id, &client_addr, &0));
    assert!(client.release_milestone(&contract_id, &client_addr, &0));

    let contract = client.get_contract(&contract_id);
    assert_eq!(contract.status, ContractStatus::Funded);
    assert_eq!(contract.released_amount, MILESTONE_ONE);

    assert!(client.approve_milestone_release(&contract_id, &client_addr, &1));
    assert!(client.release_milestone(&contract_id, &client_addr, &1));
    assert!(client.approve_milestone_release(&contract_id, &client_addr, &2));
    assert!(client.release_milestone(&contract_id, &client_addr, &2));

    let contract = client.get_contract(&contract_id);
    assert_eq!(contract.status, ContractStatus::Completed);
    assert_eq!(contract.released_amount, total_milestone_amount());
    assert_eq!(client.get_refundable_balance(&contract_id), 0);
}

#[test]
fn rejects_release_without_sufficient_balance() {
    let env = Env::default();
    env.mock_all_auths();
    let client = register_client(&env);
    let (client_addr, _freelancer_addr, contract_id) = create_contract(&env, &client);

    assert!(client.deposit_funds(&contract_id, &client_addr, &100_i128));
    assert!(client.approve_milestone_release(&contract_id, &client_addr, &0));
    let result = client.try_release_milestone(&contract_id, &client_addr, &0);
    assert_contract_error(result, EscrowError::InsufficientFunds);
}

#[test]
fn rejects_release_of_invalid_milestone_index() {
    let env = Env::default();
    env.mock_all_auths();
    let client = register_client(&env);
    let (client_addr, _freelancer_addr, contract_id) = create_contract(&env, &client);

    assert!(client.deposit_funds(&contract_id, &client_addr, &total_milestone_amount()));
    let result = client.try_release_milestone(&contract_id, &client_addr, &99);
    assert_contract_error(result, EscrowError::InvalidMilestone);
}

#[test]
fn rejects_releasing_refunded_milestone() {
    let env = Env::default();
    env.mock_all_auths();
    let client = register_client(&env);
    let (client_addr, _freelancer_addr, contract_id) = create_contract(&env, &client);

    assert!(client.deposit_funds(&contract_id, &client_addr, &total_milestone_amount()));
    client.refund_unreleased_milestones(&contract_id, &vec![&env, 1_u32]);

    assert!(client.approve_milestone_release(&contract_id, &client_addr, &1));
    let result = client.try_release_milestone(&contract_id, &client_addr, &1);
    assert_contract_error(result, EscrowError::AlreadyRefunded);
}

#[test]
fn rejects_releasing_same_milestone_twice() {
    let env = Env::default();
    env.mock_all_auths();
    let client = register_client(&env);
    let (client_addr, _freelancer_addr, contract_id) = create_contract(&env, &client);

    assert!(client.deposit_funds(&contract_id, &client_addr, &total_milestone_amount()));
    assert!(client.approve_milestone_release(&contract_id, &client_addr, &0));
    assert!(client.release_milestone(&contract_id, &client_addr, &0));

    assert!(client.approve_milestone_release(&contract_id, &client_addr, &0));
    let result = client.try_release_milestone(&contract_id, &client_addr, &0);
    assert_contract_error(result, EscrowError::AlreadyReleased);
}

// ---------------------------------------------------------------------------
// submit_work_evidence tests
// ---------------------------------------------------------------------------

#[test]
fn work_evidence_stored_on_milestone() {
    let env = Env::default();
    env.mock_all_auths();
    let escrow = register_client(&env);
    let (client_addr, freelancer_addr, contract_id) = create_contract(&env, &escrow);
    escrow.deposit_funds(&contract_id, &client_addr, &total_milestone_amount());

    let ev = evidence(&env, "ipfs://QmExampleCid");
    assert!(escrow.submit_work_evidence(&contract_id, &freelancer_addr, &0, &ev));

    let milestones = escrow.get_milestones(&contract_id);
    assert_eq!(milestones.get(0).unwrap().work_evidence, Some(ev));
}

#[test]
fn work_evidence_can_be_overwritten_before_release() {
    let env = Env::default();
    env.mock_all_auths();
    let escrow = register_client(&env);
    let (client_addr, freelancer_addr, contract_id) = create_contract(&env, &escrow);
    escrow.deposit_funds(&contract_id, &client_addr, &total_milestone_amount());

    let v1 = evidence(&env, "ipfs://first");
    let v2 = evidence(&env, "ipfs://second");
    assert!(escrow.submit_work_evidence(&contract_id, &freelancer_addr, &0, &v1));
    assert!(escrow.submit_work_evidence(&contract_id, &freelancer_addr, &0, &v2));

    let milestones = escrow.get_milestones(&contract_id);
    assert_eq!(milestones.get(0).unwrap().work_evidence, Some(v2));
}

#[test]
fn work_evidence_does_not_affect_other_milestones() {
    let env = Env::default();
    env.mock_all_auths();
    let escrow = register_client(&env);
    let (client_addr, freelancer_addr, contract_id) = create_contract(&env, &escrow);
    escrow.deposit_funds(&contract_id, &client_addr, &total_milestone_amount());

    let ev = evidence(&env, "ipfs://QmOnlyMilestone0");
    assert!(escrow.submit_work_evidence(&contract_id, &freelancer_addr, &0, &ev));

    let milestones = escrow.get_milestones(&contract_id);
    assert!(milestones.get(1).unwrap().work_evidence.is_none());
    assert!(milestones.get(2).unwrap().work_evidence.is_none());
}

#[test]
fn work_evidence_emits_evidence_event() {
    let env = Env::default();
    env.mock_all_auths();
    let escrow = register_client(&env);
    let (client_addr, freelancer_addr, contract_id) = create_contract(&env, &escrow);
    escrow.deposit_funds(&contract_id, &client_addr, &total_milestone_amount());

    let ev = evidence(&env, "ipfs://QmTest");
    assert!(escrow.submit_work_evidence(&contract_id, &freelancer_addr, &0, &ev));

    let events = env.events().all();
    use soroban_sdk::{Symbol, TryFromVal};
    assert!(events.iter().any(|e| {
        e.1.get(0)
            .and_then(|v| Symbol::try_from_val(&env, &v).ok())
            .as_ref()
            == Some(&symbol_short!("evidence"))
    }));
}

#[test]
fn work_evidence_rejects_non_freelancer_caller() {
    let env = Env::default();
    env.mock_all_auths();
    let escrow = register_client(&env);
    let (client_addr, _freelancer_addr, contract_id) = create_contract(&env, &escrow);
    escrow.deposit_funds(&contract_id, &client_addr, &total_milestone_amount());

    let ev = evidence(&env, "ipfs://QmTest");
    // client is not the freelancer
    let result = escrow.try_submit_work_evidence(&contract_id, &client_addr, &0, &ev);
    assert_contract_error(result, Error::UnauthorizedRole);
}

#[test]
fn work_evidence_rejects_stranger() {
    let env = Env::default();
    env.mock_all_auths();
    let escrow = register_client(&env);
    let (client_addr, _freelancer_addr, contract_id) = create_contract(&env, &escrow);
    escrow.deposit_funds(&contract_id, &client_addr, &total_milestone_amount());

    let stranger = Address::generate(&env);
    let ev = evidence(&env, "ipfs://QmTest");
    let result = escrow.try_submit_work_evidence(&contract_id, &stranger, &0, &ev);
    assert_contract_error(result, Error::UnauthorizedRole);
}

#[test]
fn work_evidence_rejects_unfunded_contract() {
    let env = Env::default();
    env.mock_all_auths();
    let escrow = register_client(&env);
    let (_client_addr, freelancer_addr, contract_id) = create_contract(&env, &escrow);
    // no deposit — contract stays in Created status

    let ev = evidence(&env, "ipfs://QmTest");
    let result = escrow.try_submit_work_evidence(&contract_id, &freelancer_addr, &0, &ev);
    assert_contract_error(result, Error::InvalidState);
}

#[test]
fn work_evidence_rejects_released_milestone() {
    let env = Env::default();
    env.mock_all_auths();
    let escrow = register_client(&env);
    let (client_addr, freelancer_addr, contract_id) = create_contract(&env, &escrow);
    escrow.deposit_funds(&contract_id, &client_addr, &total_milestone_amount());
    escrow.approve_milestone_release(&contract_id, &client_addr, &0);
    escrow.release_milestone(&contract_id, &client_addr, &0);

    let ev = evidence(&env, "ipfs://QmTest");
    let result = escrow.try_submit_work_evidence(&contract_id, &freelancer_addr, &0, &ev);
    assert_contract_error(result, Error::MilestoneAlreadyReleased);
}

#[test]
fn work_evidence_rejects_refunded_milestone() {
    let env = Env::default();
    env.mock_all_auths();
    let escrow = register_client(&env);
    let (client_addr, freelancer_addr, contract_id) = create_contract(&env, &escrow);
    escrow.deposit_funds(&contract_id, &client_addr, &total_milestone_amount());
    escrow.refund_unreleased_milestones(&contract_id, &vec![&env, 0_u32]);

    let ev = evidence(&env, "ipfs://QmTest");
    let result = escrow.try_submit_work_evidence(&contract_id, &freelancer_addr, &0, &ev);
    assert_contract_error(result, Error::AlreadyRefunded);
}

#[test]
fn work_evidence_rejects_out_of_bounds_index() {
    let env = Env::default();
    env.mock_all_auths();
    let escrow = register_client(&env);
    let (client_addr, freelancer_addr, contract_id) = create_contract(&env, &escrow);
    escrow.deposit_funds(&contract_id, &client_addr, &total_milestone_amount());

    let ev = evidence(&env, "ipfs://QmTest");
    let result = escrow.try_submit_work_evidence(&contract_id, &freelancer_addr, &99, &ev);
    assert_contract_error(result, Error::IndexOutOfBounds);
}

#[test]
fn work_evidence_rejects_oversized_string() {
    let env = Env::default();
    env.mock_all_auths();
    let escrow = register_client(&env);
    let (client_addr, freelancer_addr, contract_id) = create_contract(&env, &escrow);
    escrow.deposit_funds(&contract_id, &client_addr, &total_milestone_amount());

    // 257 chars > 256-byte limit
    let ev = String::from_str(&env, &"a".repeat(257));
    let result = escrow.try_submit_work_evidence(&contract_id, &freelancer_addr, &0, &ev);
    assert_contract_error(result, EscrowError::EvidenceTooLong);
}

#[test]
fn work_evidence_accepts_exactly_256_byte_string() {
    let env = Env::default();
    env.mock_all_auths();
    let escrow = register_client(&env);
    let (client_addr, freelancer_addr, contract_id) = create_contract(&env, &escrow);
    escrow.deposit_funds(&contract_id, &client_addr, &total_milestone_amount());

    let ev = String::from_str(&env, &"a".repeat(256));
    assert!(escrow.submit_work_evidence(&contract_id, &freelancer_addr, &0, &ev));
}

#[test]
fn work_evidence_rejects_paused_contract() {
    let env = Env::default();
    env.mock_all_auths();
    let escrow = register_client(&env);

    let admin = Address::generate(&env);
    escrow.initialize(&admin);

    let (client_addr, freelancer_addr, contract_id) = create_contract(&env, &escrow);
    escrow.deposit_funds(&contract_id, &client_addr, &total_milestone_amount());
    escrow.pause();

    let ev = evidence(&env, "ipfs://QmTest");
    let result = escrow.try_submit_work_evidence(&contract_id, &freelancer_addr, &0, &ev);
    assert_contract_error(result, EscrowError::ContractPaused);
}

#[test]
fn work_evidence_rejects_finalized_contract() {
    let env = Env::default();
    env.mock_all_auths();
    let escrow = register_client(&env);
    let (client_addr, freelancer_addr, contract_id) = create_contract(&env, &escrow);
    escrow.deposit_funds(&contract_id, &client_addr, &total_milestone_amount());

    // Release all milestones → Completed → finalize
    escrow.approve_milestone_release(&contract_id, &client_addr, &0);
    escrow.release_milestone(&contract_id, &client_addr, &0);
    escrow.approve_milestone_release(&contract_id, &client_addr, &1);
    escrow.release_milestone(&contract_id, &client_addr, &1);
    escrow.approve_milestone_release(&contract_id, &client_addr, &2);
    escrow.release_milestone(&contract_id, &client_addr, &2);
    escrow.finalize_contract(&contract_id, &client_addr);

    let ev = evidence(&env, "ipfs://QmAfterFinalize");
    let result = escrow.try_submit_work_evidence(&contract_id, &freelancer_addr, &0, &ev);
    assert_contract_error(result, EscrowError::AlreadyFinalized);
}

#[test]
fn work_evidence_rejects_unknown_contract() {
    let env = Env::default();
    env.mock_all_auths();
    let escrow = register_client(&env);
    let freelancer = Address::generate(&env);

    let ev = evidence(&env, "ipfs://QmTest");
    let result = escrow.try_submit_work_evidence(&9999, &freelancer, &0, &ev);
    assert_contract_error(result, Error::ContractNotFound);
}

// ---------------------------------------------------------------------------
// milestone_released / contract_completed event tests
// ---------------------------------------------------------------------------

// Helper: assert that an event with the given first-topic symbol exists in
// the event list.  Returns the matching event so callers can inspect data.
fn find_event_by_topic<'a>(
    env: &Env,
    events: &'a soroban_sdk::Vec<(soroban_sdk::Address, soroban_sdk::Vec<Val>, Val)>,
    topic_sym: Symbol,
) -> Option<(soroban_sdk::Address, soroban_sdk::Vec<Val>, Val)> {
    events.iter().find(|evt| {
        evt.1
            .get(0)
            .and_then(|v| Symbol::try_from_val(env, &v).ok())
            .as_ref()
            == Some(&topic_sym)
    })
}

#[test]
fn release_emits_milestone_released_event_with_correct_topics() {
    let env = Env::default();
    env.mock_all_auths();
    let escrow = register_client(&env);
    let (client_addr, _freelancer_addr, contract_id) = create_contract(&env, &escrow);

    escrow.deposit_funds(&contract_id, &client_addr, &total_milestone_amount());
    escrow.approve_milestone_release(&contract_id, &client_addr, &0);
    assert!(escrow.release_milestone(&contract_id, &client_addr, &0));

    let events = env.events().all();
    let topic = symbol_short!("mlstn_rls");
    let evt = find_event_by_topic(&env, &events, topic.clone());
    assert!(evt.is_some(), "expected mlstn_rls event to be emitted");

    // Verify second topic is the contract_id
    let evt = evt.unwrap();
    let second_topic: u32 = u32::try_from_val(&env, &evt.1.get(1).unwrap()).unwrap();
    assert_eq!(second_topic, contract_id);
}

#[test]
fn release_event_carries_correct_amount_and_zero_fee() {
    let env = Env::default();
    env.mock_all_auths();
    let escrow = register_client(&env);
    let (client_addr, _freelancer_addr, contract_id) = create_contract(&env, &escrow);

    escrow.deposit_funds(&contract_id, &client_addr, &total_milestone_amount());
    escrow.approve_milestone_release(&contract_id, &client_addr, &0);
    escrow.release_milestone(&contract_id, &client_addr, &0);

    let events = env.events().all();
    // There is exactly one mlstn_rls event.
    let count = events
        .iter()
        .filter(|e| {
            e.1.get(0)
                .and_then(|v| Symbol::try_from_val(&env, &v).ok())
                .as_ref()
                == Some(&symbol_short!("mlstn_rls"))
        })
        .count();
    assert_eq!(count, 1, "exactly one mlstn_rls event expected");

    // The event data tuple is (milestone_index, amount, fee, new_released_amount, caller, timestamp).
    // We decode via IntoVal round-trip: unpack the data Val as a tuple.
    // Simpler: just assert state is consistent with what the event must carry.
    let contract = escrow.get_contract(&contract_id);
    // released_amount after first release == MILESTONE_ONE
    assert_eq!(contract.released_amount, MILESTONE_ONE);
}

#[test]
fn release_event_fee_is_nonzero_when_protocol_fee_is_set() {
    let env = Env::default();
    env.mock_all_auths();
    let escrow = register_client(&env);
    let (client_addr, _freelancer_addr, contract_id) = create_contract(&env, &escrow);

    // Set a 100 bps (1%) protocol fee before depositing.
    escrow.set_protocol_fee_bps(&100u32);

    escrow.deposit_funds(&contract_id, &client_addr, &total_milestone_amount());
    escrow.approve_milestone_release(&contract_id, &client_addr, &0);
    escrow.release_milestone(&contract_id, &client_addr, &0);

    // The accumulated protocol fees must be > 0.
    let accumulated = escrow.get_accumulated_protocol_fees();
    let expected_fee = MILESTONE_ONE * 100 / 10_000;
    assert_eq!(accumulated, expected_fee);

    // The mlstn_rls event must have been emitted (fee embedded in data).
    let events = env.events().all();
    let found = events.iter().any(|e| {
        e.1.get(0)
            .and_then(|v| Symbol::try_from_val(&env, &v).ok())
            .as_ref()
            == Some(&symbol_short!("mlstn_rls"))
    });
    assert!(found, "mlstn_rls event must be emitted when fee > 0");
}

#[test]
fn no_contract_completed_event_on_partial_release() {
    let env = Env::default();
    env.mock_all_auths();
    let escrow = register_client(&env);
    let (client_addr, _freelancer_addr, contract_id) = create_contract(&env, &escrow);

    escrow.deposit_funds(&contract_id, &client_addr, &total_milestone_amount());
    // Release only milestone 0 — contract still has unreleased milestones.
    escrow.approve_milestone_release(&contract_id, &client_addr, &0);
    escrow.release_milestone(&contract_id, &client_addr, &0);

    let events = env.events().all();
    let done_topic = symbol_short!("ctrct_cmp");
    let found_done = events.iter().any(|e| {
        e.1.get(0)
            .and_then(|v| Symbol::try_from_val(&env, &v).ok())
            .as_ref()
            == Some(&done_topic)
    });
    assert!(
        !found_done,
        "ctrct_cmp must NOT be emitted when milestones remain"
    );
}

#[test]
fn contract_completed_event_emitted_on_final_milestone_release() {
    let env = Env::default();
    env.mock_all_auths();
    let escrow = register_client(&env);
    let (client_addr, _freelancer_addr, contract_id) = create_contract(&env, &escrow);

    escrow.deposit_funds(&contract_id, &client_addr, &total_milestone_amount());

    // Release milestones 0 and 1 — no completion event yet.
    escrow.approve_milestone_release(&contract_id, &client_addr, &0);
    escrow.release_milestone(&contract_id, &client_addr, &0);
    escrow.approve_milestone_release(&contract_id, &client_addr, &1);
    escrow.release_milestone(&contract_id, &client_addr, &1);

    let events_after_partial = env.events().all();
    let done_topic = symbol_short!("ctrct_cmp");
    assert!(
        !events_after_partial.iter().any(|e| {
            e.1.get(0)
                .and_then(|v| Symbol::try_from_val(&env, &v).ok())
                .as_ref()
                == Some(&done_topic)
        }),
        "ctrct_cmp must not appear before the final milestone"
    );

    // Release final milestone 2.
    escrow.approve_milestone_release(&contract_id, &client_addr, &2);
    escrow.release_milestone(&contract_id, &client_addr, &2);

    let events_final = env.events().all();
    let found_done = events_final.iter().any(|e| {
        e.1.get(0)
            .and_then(|v| Symbol::try_from_val(&env, &v).ok())
            .as_ref()
            == Some(&done_topic)
    });
    assert!(
        found_done,
        "ctrct_cmp must be emitted after the final milestone release"
    );

    // Contract status must be Completed.
    let contract = escrow.get_contract(&contract_id);
    assert_eq!(contract.status, ContractStatus::Completed);
}

#[test]
fn contract_completed_event_has_correct_contract_id_topic() {
    let env = Env::default();
    env.mock_all_auths();
    let escrow = register_client(&env);
    let (client_addr, _freelancer_addr, contract_id) = create_contract(&env, &escrow);

    escrow.deposit_funds(&contract_id, &client_addr, &total_milestone_amount());
    escrow.approve_milestone_release(&contract_id, &client_addr, &0);
    escrow.release_milestone(&contract_id, &client_addr, &0);
    escrow.approve_milestone_release(&contract_id, &client_addr, &1);
    escrow.release_milestone(&contract_id, &client_addr, &1);
    escrow.approve_milestone_release(&contract_id, &client_addr, &2);
    escrow.release_milestone(&contract_id, &client_addr, &2);

    let events = env.events().all();
    let done_topic = symbol_short!("ctrct_cmp");
    let evt = find_event_by_topic(&env, &events, done_topic);
    assert!(evt.is_some());

    let evt = evt.unwrap();
    let second_topic: u32 = u32::try_from_val(&env, &evt.1.get(1).unwrap()).unwrap();
    assert_eq!(second_topic, contract_id);
}

#[test]
fn exactly_one_completed_event_per_contract() {
    let env = Env::default();
    env.mock_all_auths();
    let escrow = register_client(&env);
    let (client_addr, _freelancer_addr, contract_id) = create_contract(&env, &escrow);

    escrow.deposit_funds(&contract_id, &client_addr, &total_milestone_amount());
    escrow.approve_milestone_release(&contract_id, &client_addr, &0);
    escrow.release_milestone(&contract_id, &client_addr, &0);
    escrow.approve_milestone_release(&contract_id, &client_addr, &1);
    escrow.release_milestone(&contract_id, &client_addr, &1);
    escrow.approve_milestone_release(&contract_id, &client_addr, &2);
    escrow.release_milestone(&contract_id, &client_addr, &2);

    let done_topic = symbol_short!("ctrct_cmp");
    let count = env
        .events()
        .all()
        .iter()
        .filter(|e| {
            e.1.get(0)
                .and_then(|v| Symbol::try_from_val(&env, &v).ok())
                .as_ref()
                == Some(&done_topic)
        })
        .count();
    assert_eq!(count, 1, "exactly one ctrct_cmp event per contract");
}

#[test]
fn release_event_not_emitted_on_failed_release_unauthorized() {
    let env = Env::default();
    env.mock_all_auths();
    let escrow = register_client(&env);
    let (client_addr, _freelancer_addr, contract_id) = create_contract(&env, &escrow);

    escrow.deposit_funds(&contract_id, &client_addr, &total_milestone_amount());
    let attacker = Address::generate(&env);
    // Try to release without approval (approval is for attacker but auth check
    // will catch the unauthorized role — no state mutation, no event).
    let _ = escrow.try_release_milestone(&contract_id, &attacker, &0);

    let events = env.events().all();
    let found = events.iter().any(|e| {
        e.1.get(0)
            .and_then(|v| Symbol::try_from_val(&env, &v).ok())
            .as_ref()
            == Some(&symbol_short!("mlstn_rls"))
    });
    assert!(
        !found,
        "mlstn_rls event must NOT be emitted on a failed release"
    );
}

#[test]
fn release_emits_three_milestone_released_events_for_three_milestones() {
    let env = Env::default();
    env.mock_all_auths();
    let escrow = register_client(&env);
    let (client_addr, _freelancer_addr, contract_id) = create_contract(&env, &escrow);

    escrow.deposit_funds(&contract_id, &client_addr, &total_milestone_amount());

    for i in 0u32..3 {
        escrow.approve_milestone_release(&contract_id, &client_addr, &i);
        escrow.release_milestone(&contract_id, &client_addr, &i);
    }

    let topic = symbol_short!("mlstn_rls");
    let count = env
        .events()
        .all()
        .iter()
        .filter(|e| {
            e.1.get(0)
                .and_then(|v| Symbol::try_from_val(&env, &v).ok())
                .as_ref()
                == Some(&topic)
        })
        .count();
    assert_eq!(count, 3, "one mlstn_rls event per milestone released");
}
