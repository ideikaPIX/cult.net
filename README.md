# CULT.NET

**CULT.NET** is an End-to-End Encrypted (E2EE) Client-Server TUI Messenger written in Rust. It utilizes WebSockets for real-time communication and RSA encryption to ensure your messages remain strictly confidential. The client features a rich Terminal User Interface (TUI) built with `ratatui`.

## Features
- **End-to-End Encryption (E2EE):** All messages are encrypted locally on the client using RSA-2048 before transmission. The server only sees and routes encrypted Base64 strings.
- **Terminal User Interface (TUI):** A clean, keyboard-driven interface for managing peers, switching accounts, and real-time chatting.
- **Non-Custodial Keys:** Private keys are generated locally and stored securely on your machine (via SQLite). They are never transmitted over the network.
- **Offline Message Queuing:** Messages sent to offline peers are temporarily queued on the server and delivered when the recipient connects.
- **Dynamic Connection:** Connect to any remote CULT.NET node directly from the UI without hardcoded IP configurations.

## Technology Stack
- **Language:** Rust
- **Async Runtime:** `tokio`
- **Networking:** `tokio-tungstenite` (WebSockets)
- **TUI:** `ratatui`, `crossterm`
- **Cryptography:** `rsa`, `sha2`, `base64`
- **Storage:** `rusqlite`

## 🚀 Getting Started

### 1. Running the Server

The server acts as a relay router and message queue. It does not read your messages.

```bash
# Build the server
cargo build --release --bin cult-server

# Run the server (defaults to 127.0.0.1:8080)
# You can override the binding address via environment variables:
CULT_SERVER_ADDR=0.0.0.0:8080 cargo run --release --bin cult-server
```
*(For production, we recommend deploying the server behind an Nginx reverse proxy).*

### 2. Running the Client

Launch the TUI client on your machine:

```bash
# Build and run the client
cargo run --release --bin cult-net
```

1. **Connect:** Upon launch, enter the IP address or domain of the CULT.NET server (e.g., `localhost:8080` or `195.133.14.56`).
2. **Register/Login:** Enter a username. If it's your first time, the client will generate an RSA keypair.
3. **Backup Key:** **⚠️ IMPORTANT:** You will be shown your private key exactly once. Copy it and keep it safe. If you lose your local SQLite database, this key is the only way to recover your account and read past messages.
4. **Chat:** Navigate the menu using the provided hotkeys (`[p]` for peers, `[a]` to add contact). Enter a peer's full address (e.g., `alice#a1b2@cult.net`) to start an encrypted chat.

## Security Disclaimer

This project is an experimental prototype. While it implements standard RSA encryption (PKCS1 v1.5), it has not been audited by a professional security firm. Do not use it for highly sensitive communications without proper independent review. 

**Remember:** Your private key is generated strictly on your client. We cannot recover your account or decrypt your messages if you lose access to your local `.cult/` directory and your private key backup.
