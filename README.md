# 🛡️ Solana-EpochRoll (v0.1.0)

**Solana-EpochRoll** is a professional-grade, lightweight monitoring service written in Rust, specifically engineered for Solana validators. It ensures absolute compliance with the Solana Foundation Delegation Program (SFDP) while providing high-precision transition metrics.

---

## 🚀 Overview

In the fast-paced Solana ecosystem, maintaining version compliance and predicting epoch transitions accurately is critical for validator health and reward eligibility. **Solana-EpochRoll** automates this by bridging the gap between on-chain reality and operational awareness.

The service performs real-time analysis of the cluster state, calculating epoch progression not just by slot count, but by true network velocity.

## ✨ Key Features

### 1. High-Precision Velocity Engine
Unlike static counters, our engine uses a **Hybrid Weighted Velocity** method:
*   **Momentum (80%)**: Analyzes the last 2000 slots (~15 mins) to capture immediate network fluctuations.
*   **Baseline (20%)**: Anchors to the overall epoch average to ensure long-term stability.
*   **Result**: Precise ETAs that match the behavior of major block explorers.

### 2. Autonomous Discovery
Designed for "zero-config" deployment. The monitor automatically queries the connected RPC to detect:
*   **Validator Identity**: Resolves the node's unique public key.
*   **Vote Account**: Dynamically finds the associated vote account for balance tracking.
*   **Cluster Drift**: Detects the lag between the Network Cluster Clock and real-world UTC, automatically correcting ETAs to match on-chain time.

### 3. Multi-Channel Alerting
Supports simultaneous reporting across enterprise communication platforms. Each provider can be individually enabled/disabled via `config.toml`:
*   **Slack**: Rich Block Kit formatting with status icons.
*   **Telegram**: Clean Markdown reports with instant mobile delivery.
*   **Discord**: Professional Embed-based alerting with status-coded colors.

### 4. Compliance & SFDP Tracking
Proactively polls the Solana Foundation API to compare your node's current version against the mandatory minimums for upcoming epochs, flagging potential non-compliance before it impacts your delegation.

---

## 🛠️ Technical Stack

*   **Language**: Rust (2021 Edition) for memory safety and performance.
*   **Async Runtime**: [Tokio](https://tokio.rs/) for efficient concurrent network I/O.
*   **Networking**: [Reqwest](https://docs.rs/reqwest/) with 10s safety timeouts.
*   **Serialization**: [Serde](https://serde.rs/) for robust JSON-RPC handling.

---

## 📦 Deployment

### Prerequisites
*   Docker & Docker Compose
*   Access to a Solana RPC (Local or Remote)
*   At least one notification channel (Slack, Telegram, or Discord)

### Quick Start
1.  Configure your `config.toml` from the provided example.
2.  Launch the container:
```bash
docker compose up -d
```

### Configuration Example (`config.toml`)
```toml
[slack]
enabled = true
webhook_url = "https://hooks.slack.com/services/..."

[telegram]
enabled = true
bot_token = "12345:ABCDE..."
chat_id = "12345678"

[discord]
enabled = false
webhook_url = ""
```

---

## 🛡️ Security
*   **Zero-Trust Identity**: No private keys are ever handled or required.
*   **Containerized**: Fully isolated runtime environment.
*   **Protocol-Native**: Communicates strictly via standard JSON-RPC over HTTPS.

---

## 📜 License
This project is open-source and available under the **MIT License**. See the [LICENSE](LICENSE) file for more details.

---

**Developed by ProviRoll for the Solana Validator Community.**