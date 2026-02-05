use crate::client::SolanaClient;
use crate::models::*;
use anyhow::Result;
use chrono::{DateTime, Duration, Utc};
use std::time::Duration as StdDuration;
use tokio::time::sleep;
use tracing::{error, info};

pub struct SFDPService {
    client: SolanaClient,
    mode: ValidatorMode,
    rpc_type: RPCType,
    slack_webhook: String,
    identity: String,
    vote_account: Option<String>,
    check_interval: StdDuration,
    report_interval: Duration,
    last_requirement_hash: u64,
    last_report_time: DateTime<Utc>,
}

impl SFDPService {
    pub fn new(
        client: SolanaClient,
        mode: ValidatorMode,
        rpc_type: RPCType,
        slack_webhook: String,
        identity: String,
        vote_account: Option<String>,
        check_secs: u64,
        report_hours: i64,
    ) -> Self {
        Self {
            client,
            mode,
            rpc_type,
            slack_webhook,
            identity,
            vote_account,
            check_interval: StdDuration::from_secs(check_secs),
            report_interval: Duration::hours(report_hours),
            last_requirement_hash: 0,
            last_report_time: Utc::now() - Duration::hours(24),
        }
    }

    fn is_compliant(&self, current: &str, required: &str) -> bool {
        let clean_curr = current
            .trim_start_matches('v')
            .split('-')
            .next()
            .unwrap_or("");
        let clean_req = required
            .trim_start_matches('v')
            .split('-')
            .next()
            .unwrap_or("");

        let curr_parts: Vec<u32> = clean_curr
            .split('.')
            .filter_map(|s| s.parse().ok())
            .collect();
        let req_parts: Vec<u32> = clean_req
            .split('.')
            .filter_map(|s| s.parse().ok())
            .collect();

        if curr_parts.is_empty() || req_parts.is_empty() {
            return false;
        }

        // Compare major.minor.patch
        curr_parts >= req_parts
    }

    fn format_duration(&self, seconds: i64) -> String {
        let days = seconds / 86400;
        let hours = (seconds % 86400) / 3600;
        let mins = (seconds % 3600) / 60;
        let secs = seconds % 60;
        if days > 0 {
            format!("{}d {}h {}m {}s", days, hours, mins, secs)
        } else if hours > 0 {
            format!("{}h {}m {}s", hours, mins, secs)
        } else {
            format!("{}m {}s", mins, secs)
        }
    }

    async fn send_slack(
        &self,
        current_ver: &str,
        info: &EpochInfo,
        requirements: &[SFDPVersionRequirement],
    ) -> Result<()> {
        let progress = (info.slot_index as f64 / info.slots_in_epoch as f64) * 100.0;

        // Precise Velocity Check
        let epoch_start_slot = info.absolute_slot.saturating_sub(info.slot_index);
        let avg_slot_time = self
            .client
            .fetch_avg_slot_time(info.absolute_slot, epoch_start_slot)
            .await
            .unwrap_or(0.38);

        // Slot Lag calculation
        let processed_slot = info.absolute_slot;
        let finalized_slot = self
            .client
            .get_slot("finalized")
            .await
            .unwrap_or(processed_slot);
        let slot_lag = processed_slot.saturating_sub(finalized_slot);

        // Fetch Additional Info
        let raw_health = self
            .client
            .get_health()
            .await
            .unwrap_or_else(|e| format!("error:{}", e));
        let (health, sync_text) = if raw_health == "ok" {
            ("ok".to_string(), String::new())
        } else if raw_health.starts_with("behind:") {
            let slots = raw_health
                .split(':')
                .nth(1)
                .unwrap_or("0")
                .parse::<u64>()
                .unwrap_or(0);
            let eta_secs = (slots as f64 * avg_slot_time) as i64;
            (
                "syncing".to_string(),
                format!(
                    "\n🔄 *Catching Up:* `{}` slots remaining (~{})",
                    slots,
                    self.format_duration(eta_secs)
                ),
            )
        } else {
            (
                "error".to_string(),
                format!("\n🔴 *RPC Connectivity Error:* `{}`", raw_health),
            )
        };

        let identity_bal_res = self.client.get_balance(&self.identity).await;
        let identity_bal_str = match identity_bal_res {
            Ok(b) => format!("{:.2} SOL", b),
            Err(_) => "Error".to_string(),
        };

        // Auto-discover Vote Account
        let vote_info = self
            .client
            .get_vote_account_by_identity(&self.identity)
            .await
            .ok()
            .flatten();
        let vote_pubkey = vote_info
            .as_ref()
            .map(|v| v.vote_pubkey.clone())
            .or(self.vote_account.clone());

        let vote_bal_str = if let Some(ref v) = vote_pubkey {
            match self.client.get_balance(v).await {
                Ok(b) => format!("{:.2} SOL", b),
                Err(_) => "Error".to_string(),
            }
        } else {
            "N/A".to_string()
        };

        let now = Utc::now();

        // Clock Drift Sync (Cluster Time vs System Time)
        let cluster_time = self
            .client
            .get_cluster_time()
            .await
            .unwrap_or(now.timestamp());
        let drift_seconds = now.timestamp() - cluster_time;
        info!("Network Clock Drift detected: {}s", drift_seconds);

        let slots_left_current = info.slots_in_epoch - info.slot_index;
        let time_left_seconds = (slots_left_current as f64 * avg_slot_time) as i64;

        // Correct ETA by subtracting the cluster drift
        let epoch_end_eta = now + Duration::seconds(time_left_seconds - drift_seconds);

        let mode_str = match self.mode {
            ValidatorMode::Agave => "AGAVE",
            ValidatorMode::Firedancer => "FIREDANCER",
        };

        info!("Building Slack payload...");
        let mut table = format!(
            "{:<6} | {:<14} | {:<14} | Status\n",
            "Epoch",
            "ETA (UTC)",
            format!("{} Req", mode_str)
        );
        table.push_str("------------------------------------------------------------\n");

        let mut overall_compliant = true;

        for req in requirements {
            let req_ver = match self.mode {
                ValidatorMode::Agave => &req.agave_min_version,
                ValidatorMode::Firedancer => &req.firedancer_min_version,
            };
            let ok = self.is_compliant(current_ver, req_ver);
            if req.epoch <= info.epoch && !ok {
                overall_compliant = false;
            }

            let status = if ok { "[ OK ]" } else { "[ !! ]" };

            let date_str = if req.epoch < info.epoch {
                "Past".to_string()
            } else if req.epoch == info.epoch {
                "Current".to_string()
            } else {
                let diff = req.epoch - info.epoch;
                let total_slots_rem = slots_left_current + ((diff - 1) * info.slots_in_epoch);
                let seconds_rem = (total_slots_rem as f64 * avg_slot_time) as i64;
                let eta = now + Duration::seconds(seconds_rem);

                if seconds_rem > 15_552_000 {
                    // > 6 months
                    eta.format("%b %d, %Y").to_string()
                } else {
                    eta.format("%b %d %H:%M").to_string()
                }
            };

            table.push_str(&format!(
                "{:<6} | {:<14} | {:<14} | {}\n",
                req.epoch, date_str, req_ver, status
            ));
        }

        let status_icon = if health == "ok" && overall_compliant {
            "🟢"
        } else if health == "syncing" {
            "⏳"
        } else if health == "error" || health != "ok" {
            "🔴"
        } else {
            "🟡"
        };

        let status_text = if health == "ok" {
            "HEALTHY"
        } else if health == "syncing" {
            "SYNCING"
        } else if health == "error" {
            "RPC ERROR"
        } else {
            "UNHEALTHY"
        };

        let compliance_text = if overall_compliant {
            "COMPLIANT"
        } else {
            "NON-COMPLIANT"
        };

        // Cluster Mismatch Warning
        let mut warning_text = sync_text;
        if !requirements.is_empty()
            && (info.epoch as i64 - requirements[0].epoch as i64).abs() > 100
        {
            warning_text.push_str(
                "\n⚠️ *Warning:* Large Epoch Delta. Are RPC and SFDP on the same cluster?",
            );
        }

        let payload = serde_json::json!({
            "text": format!("Solana-EpochRoll [{}]: {}", mode_str, status_text),
            "blocks": [
                {
                    "type": "header",
                    "text": {"type": "plain_text", "text": format!("🛡️ Solana Testnet Validator Status [{}]", mode_str)}
                },
                {
                    "type": "section",
                    "fields": [
                        { "type": "mrkdwn", "text": format!("*Status:* {} `{}`", status_icon, status_text) },
                        { "type": "mrkdwn", "text": format!("*Compliance:* `{}`", compliance_text) },
                        { "type": "mrkdwn", "text": format!("*Processed Slot:* `{}`", processed_slot) },
                        { "type": "mrkdwn", "text": format!("*Slot Lag:* `{}`", slot_lag) },
                        { "type": "mrkdwn", "text": format!("*Epoch:* `{}`", info.epoch) },
                        { "type": "mrkdwn", "text": format!("*Progress:* `{:.2}%`", progress) },
                        { "type": "mrkdwn", "text": format!("*Identity Bal:* `{}`", identity_bal_str) },
                        { "type": "mrkdwn", "text": format!("*Vote Bal:* `{}`", vote_bal_str) }
                    ]
                },
                {
                    "type": "section",
                    "text": {
                        "type": "mrkdwn",
                        "text": format!("*Ends:* `{}` UTC (`{}` left)\n*Identity:* `{}`\n*Vote:* `{}`\n*RPC:* `{}`{}",
                            epoch_end_eta.format("%b %d %H:%M"),
                            self.format_duration(time_left_seconds),
                            self.identity,
                            vote_pubkey.unwrap_or_else(|| "Unknown".to_string()),
                            self.client.get_rpc_url(),
                            warning_text)
                    }
                },
                {
                    "type": "divider"
                },
                {
                    "type": "section",
                    "text": {"type": "mrkdwn", "text": format!("```\n{}```", table)}
                },
                {
                    "type": "context",
                    "elements": [{"type": "mrkdwn", "text": format!("Method: Hybrid + Clock Sync | Velocity: {:.4}s/slot | Drift: {}s", avg_slot_time, drift_seconds)}]
                }
            ]
        });

        info!("Sending request to Slack webhook...");
        let slack_client = reqwest::Client::builder()
            .timeout(StdDuration::from_secs(10))
            .build()?;

        let resp = slack_client
            .post(&self.slack_webhook)
            .json(&payload)
            .send()
            .await?;

        info!("Slack response: {}", resp.status());
        Ok(())
    }

    async fn run_check(&mut self, force: bool) -> Result<()> {
        let mut data = self.client.fetch_sfdp_requirements().await?;
        data.sort_by_key(|r| r.epoch);

        let current_hash = fxhash::hash64(&serde_json::to_string(&data)?);
        let is_change = current_hash != self.last_requirement_hash;

        if is_change || force {
            info!("Syncing network state and transmitting report...");

            // Auto-detect identity if configured as "Unknown" or not a valid pubkey length
            if self.identity == "Unknown" || self.identity.is_empty() {
                if let Ok(id) = self.client.get_identity().await {
                    info!("Auto-detected node identity: {}", id);
                    self.identity = id;
                }
            }

            info!("Fetching version...");
            let version = self
                .client
                .get_version()
                .await
                .unwrap_or_else(|_| "Error".to_string());

            info!("Fetching epoch info...");
            let info = self.client.fetch_epoch_info().await?;

            info!("Transmitting to Slack...");
            self.send_slack(&version, &info, &data).await?;

            self.last_requirement_hash = current_hash;
            self.last_report_time = Utc::now();
        }
        Ok(())
    }

    pub async fn start(&mut self) -> Result<()> {
        info!(
            "Service booting in {:?} mode using {:?} RPC",
            self.mode, self.rpc_type
        );
        loop {
            let should_force = Utc::now() - self.last_report_time >= self.report_interval;
            if let Err(e) = self.run_check(should_force).await {
                error!("Loop error: {}", e);
            }
            sleep(self.check_interval).await;
        }
    }
}
