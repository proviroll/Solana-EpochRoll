# Solana-EpochRoll (Rust)

**Solana-EpochRoll** is a high-performance compliance monitor designed to ensure your validator remains aligned with SFDP version requirements. Rewritten in Rust for maximum reliability and minimal footprint, it provides real-time Slack alerts and heartbeat reporting for both Agave and Firedancer nodes.

## ⚡ Why Rust?
- **Zero-cost abstractions:** Highly efficient monitoring with negligible CPU/Memory impact.
- **Async Reliability:** Built on `tokio` for robust network operations.
- **Type Safety:** Ensures exact handling of network and version data.

## 🚀 Quick Start (Docker)

### 1. Setup
```bash
cp .env.example .env
# Edit .env with your Slack Webhook and Mode
```

### 2. Deploy
```bash
docker-compose up -d
```

## ⚙️ Configuration (`.env`)

| Variable | Description | Default |
| :--- | :--- | :--- |
| `VALIDATOR_MODE` | `agave` or `firedancer` | `firedancer` |
| `RPC_TYPE` | `local` or `remote` | `remote` |
| `RPC_URL` | Solana RPC endpoint | `https://api.testnet.solana.com` |
| `SLACK_WEBHOOK_URL` | Incoming Webhook URL | `REQUIRED` |
| `VALIDATOR_IDENTITY` | Validator Pubkey | `Unknown` |
| `REPORT_INTERVAL_HOURS`| Heartbeat frequency | `8` |

## 📄 License
MIT License - Open Source by ProviRoll.
