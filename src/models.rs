use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct SFDPVersionRequirement {
    pub epoch: u64,
    pub agave_min_version: String,
    pub firedancer_min_version: String,
}

#[derive(Debug, Deserialize)]
pub struct SFDPResponse {
    pub data: Vec<SFDPVersionRequirement>,
}

#[derive(Debug, Deserialize)]
pub struct EpochInfo {
    pub epoch: u64,
    #[serde(rename = "slotsInEpoch")]
    pub slots_in_epoch: u64,
    #[serde(rename = "slotIndex")]
    pub slot_index: u64,
    #[serde(rename = "absoluteSlot")]
    pub absolute_slot: u64,
}

#[derive(Debug, Deserialize)]
pub struct BalanceResponse {
    pub value: u64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VoteAccountInfo {
    pub vote_pubkey: String,
    pub node_pubkey: String,
}

#[derive(Debug, Deserialize)]
pub struct VoteAccountsResponse {
    pub current: Vec<VoteAccountInfo>,
    pub delinquent: Vec<VoteAccountInfo>,
}

#[derive(Debug, Deserialize)]
pub struct RPCResponse<T> {
    pub result: Option<T>,
    pub error: Option<RPCError>,
}

#[derive(Debug, Deserialize)]
pub struct RPCError {
    pub message: String,
    pub data: Option<serde_json::Value>,
}

#[derive(Debug, Clone)]
pub struct ReportData {
    pub mode_str: String,
    pub status_icon: String,
    pub status_text: String,
    pub compliance_text: String,
    pub epoch: u64,
    pub progress: f64,
    pub processed_slot: u64,
    pub slot_lag: u64,
    pub identity_bal: String,
    pub vote_bal: String,
    pub eta_str: String,
    pub time_left: String,
    pub identity: String,
    pub vote_pubkey: String,
    pub rpc_url: String,
    pub warning_text: String,
    pub avg_slot_time: f64,
    pub current_ver: String,
    pub table: String,
    pub drift_seconds: i64,
}

#[derive(Debug, Deserialize)]
pub struct VersionResult {
    #[serde(rename = "solana-core")]
    pub solana_core: String,
}

#[derive(Debug, Clone)]
pub enum ValidatorMode {
    Agave,
    Firedancer,
}

impl From<String> for ValidatorMode {
    fn from(s: String) -> Self {
        match s.to_lowercase().as_str() {
            "agave" => ValidatorMode::Agave,
            _ => ValidatorMode::Firedancer,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum RPCType {
    Local,
    Remote,
}

impl From<String> for RPCType {
    fn from(s: String) -> Self {
        match s.to_lowercase().as_str() {
            "local" => RPCType::Local,
            _ => RPCType::Remote,
        }
    }
}
