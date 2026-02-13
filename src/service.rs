pub mod notifications;

use crate::client::SolanaClient;
use crate::models::*;
use crate::service::notifications::NotificationProvider;
use crate::utils::{format_duration, is_compliant};
use anyhow::Result;
use chrono::{DateTime, Duration, Utc};
use std::time::Duration as StdDuration;
use tokio::time::sleep;
use tracing::{error, info};

pub struct SFDPService {
    client: SolanaClient,
    mode: ValidatorMode,
    rpc_type: RPCType,
    identity: String,
    vote_account: Option<String>,
    check_interval: StdDuration,
    report_interval: Duration,
    last_requirement_hash: u64,
    last_report_time: DateTime<Utc>,
    notification_providers: Vec<Box<dyn NotificationProvider>>,
}

impl SFDPService {
    pub fn new(
        client: SolanaClient,
        mode: ValidatorMode,
        rpc_type: RPCType,
        identity: String,
        vote_account: Option<String>,
        check_secs: u64,
        report_hours: i64,
        notification_providers: Vec<Box<dyn NotificationProvider>>,
    ) -> Self {
        Self {
            client,
            mode,
            rpc_type,
            identity,
            vote_account,
            check_interval: StdDuration::from_secs(check_secs),
            report_interval: Duration::hours(report_hours),
            last_requirement_hash: 0,
            last_report_time: Utc::now() - Duration::hours(24),
            notification_providers,
        }
    }

    async fn broadcast_report(
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
                    format_duration(eta_secs)
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
        let vote_pubkey_str = vote_pubkey.clone().unwrap_or_else(|| "Unknown".to_string());

        let vote_bal_str = if let Some(ref v) = vote_pubkey {
            match self.client.get_balance(v).await {
                Ok(b) => format!("{:.2} SOL", b),
                Err(_) => "Error".to_string(),
            }
        } else {
            "N/A".to_string()
        };

        let now = Utc::now();

        let slots_left_current = info.slots_in_epoch - info.slot_index;
        let time_left_seconds = (slots_left_current as f64 * avg_slot_time) as i64;

        // Correct ETA: Simple duration addition based on actual slot velocity
        let epoch_end_eta = now + Duration::seconds(time_left_seconds);

        let mode_str = match self.mode {
            ValidatorMode::Agave => "AGAVE",
            ValidatorMode::Firedancer => "FIREDANCER",
        };

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
            let ok = is_compliant(current_ver, req_ver);
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

        // --- Enhanced Maintenance Advisor Logic ---
        let mut maintenance_msg = None;
        if !overall_compliant {
            if let Ok(schedule) = self.client.get_leader_schedule(&self.identity).await {
                let epoch_start_slot = info.absolute_slot.saturating_sub(info.slot_index);
                let absolute_schedule: Vec<u64> =
                    schedule.iter().map(|s| epoch_start_slot + s).collect();
                let future_blocks: Vec<u64> = absolute_schedule
                    .iter()
                    .cloned()
                    .filter(|s| *s > processed_slot)
                    .collect();

                if future_blocks.is_empty() {
                    maintenance_msg = Some("🚨 *ACTION REQUIRED:* Your software is out of date. Since you have no more blocks assigned for this epoch, *UPGRADE IMMEDIATELY* to restore compliance.".to_string());
                } else {
                    let next_block = future_blocks[0];
                    let mins_until_next =
                        ((next_block - processed_slot) as f64 * avg_slot_time / 60.0) as i64;

                    // Group clusters for display
                    let mut clusters = Vec::new();
                    let mut temp = vec![future_blocks[0]];
                    for i in 1..future_blocks.len() {
                        if future_blocks[i] == future_blocks[i - 1] + 1 {
                            temp.push(future_blocks[i]);
                        } else {
                            clusters.push(temp);
                            temp = vec![future_blocks[i]];
                            if clusters.len() >= 4 {
                                break;
                            }
                        }
                    }
                    if clusters.len() < 4 {
                        clusters.push(temp);
                    }

                    let mut schedule_text = String::from("\n*Upcoming Workload:*");
                    for c in &clusters {
                        let c_start = c[0];
                        let c_eta_utc = now
                            + Duration::seconds(
                                ((c_start - processed_slot) as f64 * avg_slot_time) as i64,
                            );
                        schedule_text.push_str(&format!(
                            "\n• Slot `{}` ({} blocks) at `{}` UTC",
                            c_start,
                            c.len(),
                            c_eta_utc.format("%H:%M")
                        ));
                    }

                    if mins_until_next >= 25 {
                        let deadline_utc = now + Duration::minutes(mins_until_next - 5);
                        maintenance_msg = Some(format!(
                            "⚠️ *SOFTWARE UPGRADE RECOMMENDED:* You are currently non-compliant.\n✅ *Instruction:* You have a `{}` minute window. *Safe to upgrade NOW.*\n🚨 *Deadline:* You must be back online and synced by *{} UTC* to avoid missing your next assignment.{}",
                            mins_until_next,
                            deadline_utc.format("%H:%M"),
                            schedule_text
                        ));
                    } else {
                        let cluster_end = clusters[0].last().unwrap();
                        let next_window_start_utc = now
                            + Duration::seconds(
                                ((*cluster_end - processed_slot) as f64 * avg_slot_time) as i64 + 30,
                            );

                        let next_gap_mins = if clusters.len() > 1 {
                            let gap_slots = clusters[1][0] - clusters[0].last().unwrap();
                            (gap_slots as f64 * avg_slot_time / 60.0) as i64
                        } else {
                            999
                        };

                        maintenance_msg = Some(format!(
                            "⚠️ *SOFTWARE UPGRADE REQUIRED:* You are currently non-compliant.\n⏳ *Instruction:* DO NOT restart yet. Blocks are imminent in `{}` minutes.\n✅ *Best Action:* Wait until your current assignment ends. *Start upgrade at {} UTC.*\n💡 *Note:* The next window will be `{}` minutes long, which is safe for a Frankendancer restart.{}",
                            mins_until_next,
                            next_window_start_utc.format("%H:%M"),
                            next_gap_mins,
                            schedule_text
                        ));
                    }
                }
            }
        }

        let mut warning_text = sync_text;
        if !requirements.is_empty()
            && (info.epoch as i64 - requirements[0].epoch as i64).abs() > 100
        {
            warning_text.push_str(
                "\n⚠️ *Warning:* Large Epoch Delta. Are RPC and SFDP on the same cluster?",
            );
        }

        let report_data = ReportData {
            mode_str: mode_str.to_string(),
            status_icon: status_icon.to_string(),
            status_text: status_text.to_string(),
            compliance_text: compliance_text.to_string(),
            epoch: info.epoch,
            progress,
            processed_slot,
            slot_lag,
            identity_bal: identity_bal_str,
            vote_bal: vote_bal_str,
            eta_str: epoch_end_eta.format("%b %d %H:%M").to_string(),
            time_left: format_duration(time_left_seconds),
            identity: self.identity.clone(),
            vote_pubkey: vote_pubkey_str,
            rpc_url: self.client.get_rpc_url().to_string(),
            warning_text,
            avg_slot_time,
            current_ver: current_ver.to_string(),
            table,
            maintenance_msg,
        };

        for provider in &self.notification_providers {
            if let Err(e) = provider.notify(&report_data).await {
                error!("Notification failure: {}", e);
            }
        }

        Ok(())
    }

    async fn run_check(&mut self, force: bool) -> Result<()> {
        let mut data = self.client.fetch_sfdp_requirements().await?;
        data.sort_by_key(|r| r.epoch);

        let current_hash = fxhash::hash64(&serde_json::to_string(&data)?);
        let is_change = current_hash != self.last_requirement_hash;

        if is_change || force {
            info!("Syncing network state and transmitting report...");

            // Auto-detect identity if not explicitly provided
            let id_lower = self.identity.to_lowercase();
            if id_lower.is_empty() || id_lower == "auto" || id_lower == "unknown" {
                if let Ok(id) = self.client.get_identity().await {
                    info!("Auto-detected node identity: {}", id);
                    self.identity = id;
                }
            }

            info!("Fetching version...");
            let version = self
                .client
                .get_validator_version(&self.identity)
                .await
                .unwrap_or_else(|_| "Unknown".to_string());

            info!("Fetching epoch info...");
            let info = self.client.fetch_epoch_info().await?;

            info!("Transmitting broadcast report...");
            self.broadcast_report(&version, &info, &data).await?;

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
