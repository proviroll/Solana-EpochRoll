use crate::models::*;
use anyhow::{anyhow, Result};

pub struct SolanaClient {
    rpc_url: String,
    sfdp_api_url: String,
    http_client: reqwest::Client,
}

impl SolanaClient {
    pub fn new(rpc_url: String, sfdp_api_url: String) -> Self {
        Self {
            rpc_url,
            sfdp_api_url,
            http_client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(10))
                .build()
                .unwrap_or_default(),
        }
    }

    pub fn get_rpc_url(&self) -> &str {
        &self.rpc_url
    }

    pub async fn fetch_sfdp_requirements(&self) -> Result<Vec<SFDPVersionRequirement>> {
        let resp = self
            .http_client
            .get(&self.sfdp_api_url)
            .send()
            .await?
            .json::<SFDPResponse>()
            .await?;
        Ok(resp.data)
    }

    pub async fn fetch_epoch_info(&self) -> Result<EpochInfo> {
        let resp = self
            .http_client
            .post(&self.rpc_url)
            .json(&serde_json::json!({"jsonrpc":"2.0","id":1, "method":"getEpochInfo"}))
            .send()
            .await?
            .json::<RPCResponse<EpochInfo>>()
            .await?;

        if let Some(error) = resp.error {
            return Err(anyhow!("RPC Error (getEpochInfo): {}", error.message));
        }
        resp.result
            .ok_or_else(|| anyhow!("No result in getEpochInfo response"))
    }

    pub async fn get_block_time(&self, slot: u64) -> Result<i64> {
        let resp = self.http_client.post(&self.rpc_url)
            .json(&serde_json::json!({"jsonrpc":"2.0","id":1, "method":"getBlockTime", "params": [slot]}))
            .send()
            .await?
            .json::<RPCResponse<Option<i64>>>()
            .await?;

        if let Some(error) = resp.error {
            return Err(anyhow!(
                "RPC Error (getBlockTime) for slot {}: {}",
                slot,
                error.message
            ));
        }

        resp.result
            .flatten()
            .ok_or_else(|| anyhow!("No time for slot {}", slot))
    }

    pub async fn fetch_avg_slot_time(
        &self,
        current_slot: u64,
        epoch_start_slot: u64,
    ) -> Result<f64> {
        // Hybrid Precision Method (Reverted to 10m drift version):
        // 1. Recent Velocity (Last 2000 slots ~15 mins) - High Reactivity
        // 2. Epoch Velocity (Since epoch start) - High Stability

        let slots_in_epoch = current_slot.saturating_sub(epoch_start_slot);

        // Fetch Recent Velocity
        let recent_lookback = 2000;
        let t_now = self.get_block_time(current_slot).await?;
        let t_recent = self.get_block_time(current_slot - recent_lookback).await?;
        let v_recent = (t_now - t_recent) as f64 / recent_lookback as f64;

        // If epoch is very young, just use recent velocity
        if slots_in_epoch < 5000 {
            return Ok(v_recent.clamp(0.3, 0.8));
        }

        // Fetch Epoch Velocity
        let t_start = self.get_block_time(epoch_start_slot).await?;
        let v_epoch = (t_now - t_start) as f64 / slots_in_epoch as f64;

        // Weighted Hybrid: 80% Recent Momentum + 20% Epoch Baseline
        let v_hybrid = (v_recent * 0.8) + (v_epoch * 0.2);

        Ok(v_hybrid.clamp(0.3, 0.8))
    }

    pub async fn get_health(&self) -> Result<String> {
        let resp = self
            .http_client
            .post(&self.rpc_url)
            .json(&serde_json::json!({"jsonrpc":"2.0","id":1, "method":"getHealth"}))
            .send()
            .await?
            .json::<RPCResponse<String>>()
            .await?;

        if let Some(error) = resp.error {
            // Check if it's "behind" and extract slot count
            if let Some(data) = error.data {
                if let Some(behind) = data.get("numSlotsBehind").and_then(|v| v.as_u64()) {
                    return Ok(format!("behind:{}", behind));
                }
            }
            return Ok(error.message);
        }
        Ok(resp.result.unwrap_or_else(|| "ok".to_string()))
    }

    pub async fn get_balance(&self, pubkey: &str) -> Result<f64> {
        let resp = self.http_client.post(&self.rpc_url)
            .json(&serde_json::json!({"jsonrpc":"2.0","id":1, "method":"getBalance", "params": [pubkey]}))
            .send()
            .await?
            .json::<RPCResponse<BalanceResponse>>()
            .await?;

        if let Some(error) = resp.error {
            return Err(anyhow!(
                "RPC Error (getBalance) for {}: {}",
                pubkey,
                error.message
            ));
        }
        let lamports = resp.result.map(|r| r.value).unwrap_or(0);
        Ok(lamports as f64 / 1_000_000_000.0)
    }

    pub async fn get_vote_account_by_identity(
        &self,
        identity: &str,
    ) -> Result<Option<VoteAccountInfo>> {
        let resp = self
            .http_client
            .post(&self.rpc_url)
            .json(&serde_json::json!({
                "jsonrpc":"2.0","id":1,
                "method":"getVoteAccounts",
                "params": [{"votePubkey": null}]
            }))
            .send()
            .await?
            .json::<RPCResponse<VoteAccountsResponse>>()
            .await?;

        if let Some(error) = resp.error {
            return Err(anyhow!("RPC Error (getVoteAccounts): {}", error.message));
        }

        if let Some(res) = resp.result {
            // Check current validators
            if let Some(vote) = res.current.into_iter().find(|v| v.node_pubkey == identity) {
                return Ok(Some(vote));
            }
            // Check delinquent validators
            if let Some(vote) = res
                .delinquent
                .into_iter()
                .find(|v| v.node_pubkey == identity)
            {
                return Ok(Some(vote));
            }
        }
        Ok(None)
    }

    pub async fn get_slot(&self, commitment: &str) -> Result<u64> {
        let resp = self
            .http_client
            .post(&self.rpc_url)
            .json(&serde_json::json!({
                "jsonrpc":"2.0","id":1,
                "method":"getSlot",
                "params": [{"commitment": commitment}]
            }))
            .send()
            .await?
            .json::<RPCResponse<u64>>()
            .await?;

        resp.result
            .ok_or_else(|| anyhow!("No slot returned for {}", commitment))
    }

    pub async fn get_identity(&self) -> Result<String> {
        let resp = self
            .http_client
            .post(&self.rpc_url)
            .json(&serde_json::json!({"jsonrpc":"2.0","id":1, "method":"getIdentity"}))
            .send()
            .await?
            .json::<RPCResponse<serde_json::Value>>()
            .await?;

        if let Some(error) = resp.error {
            return Err(anyhow!("RPC Error (getIdentity): {}", error.message));
        }

        resp.result
            .and_then(|r| {
                r.get("identity")
                    .and_then(|i| i.as_str().map(|s| s.to_string()))
            })
            .ok_or_else(|| anyhow!("No identity found in RPC response"))
    }

    pub async fn get_version(&self) -> Result<String> {
        let resp = self
            .http_client
            .post(&self.rpc_url)
            .json(&serde_json::json!({"jsonrpc":"2.0","id":1, "method":"getVersion"}))
            .send()
            .await?
            .json::<RPCResponse<VersionResult>>()
            .await?;

        if let Some(error) = resp.error {
            return Err(anyhow!("RPC Error (getVersion): {}", error.message));
        }
        resp.result
            .map(|r| r.solana_core)
            .ok_or_else(|| anyhow!("No result in getVersion response"))
    }

    pub async fn get_leader_schedule(&self, identity: &str) -> Result<Vec<u64>> {
        let resp = self
            .http_client
            .post(&self.rpc_url)
            .json(&serde_json::json!({
                "jsonrpc": "2.0", "id": 1,
                "method": "getLeaderSchedule",
                "params": [serde_json::Value::Null, {"identity": identity}]
            }))
            .send()
            .await?
            .json::<RPCResponse<serde_json::Value>>()
            .await?;

        if let Some(error) = resp.error {
            return Err(anyhow!("RPC Error (getLeaderSchedule): {}", error.message));
        }

        let schedule = resp
            .result
            .and_then(|r| r.get(identity).cloned())
            .and_then(|v| serde_json::from_value::<Vec<u64>>(v).ok())
            .unwrap_or_default();

        Ok(schedule)
    }

    pub async fn get_validator_version(&self, identity: &str) -> Result<String> {
        let resp = self
            .http_client
            .post(&self.rpc_url)
            .json(&serde_json::json!({
                "jsonrpc": "2.0", "id": 1,
                "method": "getClusterNodes"
            }))
            .send()
            .await?
            .json::<RPCResponse<Vec<serde_json::Value>>>()
            .await?;

        if let Some(error) = resp.error {
            return Err(anyhow!("RPC Error (getClusterNodes): {}", error.message));
        }

        let nodes = resp.result.unwrap_or_default();
        for node in nodes {
            if node.get("pubkey").and_then(|v| v.as_str()) == Some(identity) {
                return node
                    .get("version")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
                    .ok_or_else(|| anyhow!("No version field for node"));
            }
        }

        Err(anyhow!("Validator identity not found in cluster nodes"))
    }
}
