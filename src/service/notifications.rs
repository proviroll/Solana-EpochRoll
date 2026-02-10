use anyhow::Result;
use async_trait::async_trait;
use crate::models::ReportData;
use reqwest::Client;
use serde_json::json;

#[async_trait]
pub trait NotificationProvider: Send + Sync {
    async fn notify(&self, data: &ReportData) -> Result<()>;
}

pub struct SlackProvider {
    webhook_url: String,
    client: Client,
}

impl SlackProvider {
    pub fn new(webhook_url: String) -> Self {
        Self {
            webhook_url,
            client: Client::new(),
        }
    }
}

#[async_trait]
impl NotificationProvider for SlackProvider {
    async fn notify(&self, data: &ReportData) -> Result<()> {
        let payload = json!({
            "text": format!("Solana-EpochRoll [{}]: {}", data.mode_str, data.status_text),
            "blocks": [
                {
                    "type": "header",
                    "text": {"type": "plain_text", "text": format!("🛡️ Solana Testnet Validator Status [{}]", data.mode_str)}
                },
                {
                    "type": "section",
                    "fields": [
                        { "type": "mrkdwn", "text": format!("*Status:* {} `{}`", data.status_icon, data.status_text) },
                        { "type": "mrkdwn", "text": format!("*Compliance:* `{}`", data.compliance_text) },
                        { "type": "mrkdwn", "text": format!("*Processed Slot:* `{}`", data.processed_slot) },
                        { "type": "mrkdwn", "text": format!("*Slot Lag:* `{}`", data.slot_lag) },
                        { "type": "mrkdwn", "text": format!("*Epoch:* `{}`", data.epoch) },
                        { "type": "mrkdwn", "text": format!("*Progress:* `{:.2}%`", data.progress) },
                        { "type": "mrkdwn", "text": format!("*Identity Bal:* `{}`", data.identity_bal) },
                        { "type": "mrkdwn", "text": format!("*Vote Bal:* `{}`", data.vote_bal) }
                    ]
                },
                {
                    "type": "section",
                    "text": {
                        "type": "mrkdwn", 
                        "text": format!("*Ends:* `{}` UTC (`{}` left)\n*Identity:* `{}`\n*Vote:* `{}`\n*RPC:* `{}`{}", 
                            data.eta_str,
                            data.time_left,
                            data.identity,
                            data.vote_pubkey,
                            data.rpc_url,
                            data.warning_text)
                    }
                },
                {
                    "type": "divider"
                },
                {
                    "type": "section",
                    "text": {"type": "mrkdwn", "text": format!("```\n{}```", data.table)}
                },
                {
                    "type": "context",
                    "elements": [{"type": "mrkdwn", "text": format!("Method: Hybrid + Clock Sync | Velocity: {:.4}s/slot | Drift: {}s | Detected: {}", data.avg_slot_time, data.drift_seconds, data.current_ver)}]
                }
            ]
        });

        self.client.post(&self.webhook_url).json(&payload).send().await?;
        tracing::info!("Slack notification sent successfully");
        Ok(())
    }
}

pub struct TelegramProvider {
    bot_token: String,
    chat_id: String,
    client: Client,
}

impl TelegramProvider {
    pub fn new(bot_token: String, chat_id: String) -> Self {
        Self {
            bot_token,
            chat_id,
            client: Client::new(),
        }
    }
}

#[async_trait]
impl NotificationProvider for TelegramProvider {
    async fn notify(&self, data: &ReportData) -> Result<()> {
        let text = format!(
            "🛡️ *Solana Testnet Validator Status [{}]*\n\n\
            *Status:* {} `{}`\n\
            *Compliance:* `{}`\n\
            *Processed Slot:* `{}`\n\
            *Slot Lag:* `{}`\n\
            *Epoch:* `{}`\n\
            *Progress:* `{:.2}%`\n\
            *Identity Bal:* `{}`\n\
            *Vote Bal:* `{}`\n\n\
            *Ends:* `{}` UTC (`{}` left)\n\
            *Identity:* `{}`\n\
            *Vote:* `{}`\n\
            *RPC:* `{}`\n\
            {}\n\n\
            ```\n{}```\n\
            _Method: Hybrid + Clock Sync | Velocity: {:.4}s/slot | Drift: {}s | Detected: {}_",
            data.mode_str, data.status_icon, data.status_text, data.compliance_text, 
            data.processed_slot, data.slot_lag, data.epoch, data.progress, 
            data.identity_bal, data.vote_bal, data.eta_str, data.time_left, 
            data.identity, data.vote_pubkey, data.rpc_url, data.warning_text,
            data.table, data.avg_slot_time, data.drift_seconds, data.current_ver
        );

        let url = format!("https://api.telegram.org/bot{}/sendMessage", self.bot_token);
        self.client.post(&url)
            .json(&json!({
                "chat_id": self.chat_id,
                "text": text,
                "parse_mode": "Markdown"
            }))
            .send()
            .await?;
        tracing::info!("Telegram notification sent successfully");
        Ok(())
    }
}

pub struct DiscordProvider {
    webhook_url: String,
    client: Client,
}

impl DiscordProvider {
    pub fn new(webhook_url: String) -> Self {
        Self {
            webhook_url,
            client: Client::new(),
        }
    }
}

#[async_trait]
impl NotificationProvider for DiscordProvider {
    async fn notify(&self, data: &ReportData) -> Result<()> {
        let description = format!(
            "**Status:** {} `{}`\n\
            **Compliance:** `{}`\n\
            **Processed Slot:** `{}`\n\
            **Slot Lag:** `{}`\n\
            **Epoch:** `{}`\n\
            **Progress:** `{:.2}%`\n\
            **Identity Bal:** `{}`\n\
            **Vote Bal:** `{}`\n\n\
            **Ends:** `{}` UTC (`{}` left)\n\
            **Identity:** `{}`\n\
            **Vote:** `{}`\n\
            **RPC:** `{}`\n\
            {}",
            data.status_icon, data.status_text, data.compliance_text, 
            data.processed_slot, data.slot_lag, data.epoch, data.progress, 
            data.identity_bal, data.vote_bal, data.eta_str, data.time_left, 
            data.identity, data.vote_pubkey, data.rpc_url, data.warning_text
        );

        let payload = json!({
            "content": null,
            "embeds": [{
                "title": format!("🛡️ Solana Testnet Validator Status [{}]", data.mode_str),
                "description": description,
                "color": if data.status_text == "HEALTHY" { 5763719 } else { 15548997 },
                "fields": [
                    {
                        "name": "Compliance Table",
                        "value": format!("```\n{}```", data.table)
                    }
                ],
                "footer": {
                    "text": format!("Method: Hybrid + Clock Sync | Velocity: {:.4}s/slot | Drift: {}s | Detected: {}", data.avg_slot_time, data.drift_seconds, data.current_ver)
                }
            }]
        });

        self.client.post(&self.webhook_url).json(&payload).send().await?;
        tracing::info!("Discord notification sent successfully");
        Ok(())
    }
}