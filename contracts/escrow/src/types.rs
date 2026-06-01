use soroban_sdk::{contracterror, contracttype, Address, String};

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DataKey {
    // Admin / pause / emergency
    Initialized,
    Contract(u32),
    NextContractId,
    Admin,
    ReadinessChecklist,
    GovernedParameters,
    ProtocolFeeBps,
    PendingAdmin,
    InitializedV2,
    /// Stores milestone approval flags (contract_id, milestone_index) -> MilestoneApprovals
    /// Stored in temporary storage with TTL for expiry grace period
    MilestoneApprovals(u32, u32),
    Finalization(u32),
    PendingClientMigration(u32),
    ReputationIssued(u32),
    MilestoneReleased(u32, u32),
}

#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ContractStatus {
    Created = 0,
    Funded = 1,
    Completed = 2,
    Disputed = 3,
    Refunded = 4,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Milestone {
    pub amount: i128,
    pub released: bool,
    pub refunded: bool,
    pub work_evidence: Option<String>,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MilestoneSchedule {
    pub due_date: Option<u64>,
    pub title: Option<String>,
    pub description: Option<String>,
    pub updated_at: u64,
}

/// Readiness checklist stored under [`DataKey::ReadinessChecklist`].
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReadinessChecklist {
    /// `true` after `initialize` has been called successfully.
    pub initialized: bool,
    /// `true` after protocol governance parameters have been set.
    pub governed_params_set: bool,
    /// `true` after an emergency control operation has been invoked.
    pub emergency_controls_enabled: bool,
}

impl Default for ReadinessChecklist {
    fn default() -> Self {
        ReadinessChecklist {
            initialized: false,
            governed_params_set: false,
            emergency_controls_enabled: false,
        }
    }
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GovernedParameters {
    pub protocol_fee_bps: u32,
    pub max_escrow_total_stroops: i128,
}

// ─── Indexer summary types ────────────────────────────────────────────────────

pub const CONTRACT_SUMMARY_SCHEMA_VERSION: u32 = 1;

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MilestoneSummary {
    pub index: u32,
    pub amount: i128,
    pub released: bool,
    pub refunded: bool,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContractSummary {
    pub schema_version: u32,
    pub client: Address,
    pub freelancer: Address,
    pub arbiter: Option<Address>,
    pub status: ContractStatus,
    pub reputation_issued: bool,
    pub total_amount: i128,
    pub funded_amount: i128,
    pub released_amount: i128,
    pub refundable_balance: i128,
    pub released_milestone_count: u32,
    pub milestones: soroban_sdk::Vec<MilestoneSummary>,
}

#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DepositMode {
    ExactTotal = 0,
    Incremental = 1,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DisputeResolution {
    FullRefund = 0,
    PartialRefund = 1,
    FullPayout = 2,
    Split(i128, i128),
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FinalizationRecord {
    pub finalizer: Address,
    pub timestamp: u64,
    pub summary: ContractSummary,
}

#[contracttype]
#[derive(Clone, Debug)]
pub struct Contract {
    pub client: soroban_sdk::Address,
    pub freelancer: soroban_sdk::Address,
    pub arbiter: Option<soroban_sdk::Address>,
    pub status: ContractStatus,
    pub funded_amount: i128,
    pub released_amount: i128,
    pub refunded_amount: i128,
    pub release_authorization: ReleaseAuthorization,
    pub total_deposited: i128,
}

/// Defines who can approve milestone releases
#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReleaseAuthorization {
    /// Only client can approve
    ClientOnly = 0,
    /// Either client or arbiter can approve
    ClientAndArbiter = 1,
    /// Only arbiter can approve
    ArbiterOnly = 2,
    /// Both client and freelancer must approve
    MultiSig = 3,
}

/// Tracks approval status for a milestone
/// Stored in temporary storage with TTL for expiry grace period
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MilestoneApprovals {
    pub client_approved: bool,
    pub freelancer_approved: bool,
    pub arbiter_approved: bool,
}
