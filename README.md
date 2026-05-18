<p align="center">
  <img src="banner.png" width="100%">
</p>

---

# CULT.NET

🔐 **CULT.NET** is an ultra-compact, high-security, two-pane terminal messenger written in Rust. It features an end-to-end encrypted hybrid cryptographic core, an asynchronous networking engine with automatic failovers, and a highly optimized monocolor TUI dashboard tailored specifically for narrow viewports, system administrators, and power users (WSL, Termux, Linux consoles).

---

## Architecture Overview

### Hybrid Cryptography Core
To ensure maximum performance without compromising security, CULT.NET uses a dual-layer hybrid encryption pipeline:
* **Asymmetric Layer (RSA-2048):** Used for initial key exchange and identity verification between peers.
* **Symmetric Layer (AES-256-GCM):** Once a secure session is initiated, messages are packed with a unique `nonce` and encrypted via hardware-accelerated AES.
* **Fallback Protocol:** If an incoming payload fails to decrypt via AES (e.g., due to session desynchronization), the client automatically triggers an RSA decryption fallback to process identity payloads without dropping the connection.

### Network Gateway & Persistence
* **Resilient Connection Handling:** The client utilizes a 3-attempt connection loop before prompting a fallback state.
* **Asynchronous Background Pinger:** An active background scanner pings configured peers every 20 seconds even before entering the active peer view (`AppMode::Peers`), ensuring live status metrics immediately upon UI initialization.
* **Local Storage:** Message history and credential indexing are securely maintained via a local SQLite database infrastructure.

---

## Project Structure
```
cult.net/
 ├── .gitignore               # Configured ignore list (filters out DBs, keys, logs, and build artifacts)
 ├── Cargo.toml               # Project manifest with dependencies (tokio, ratatui, ring, aes-gcm, rusqlite)
 ├── Cargo.lock               # Locked versions of all Rust crates
 ├── README.md                # Minimalist project description and deployment/startup guidelines
 ├── CULT.md                  # Internal project documentation and development roadmap
 ├── config.toml.example      # Configuration template file
 ├── cult-server.service      # Systemd unit file to deploy the server as a background daemon on Linux
 ├── nginx.conf               # Nginx configuration template
 │
 └── src/                     # Rust source code
      ├── main.rs             # Default application entry point
      ├── shared.rs           # Shared data structures for client and server (e.g., ClientMessage / ServerResponse enums)
      │
      ├── client/             # 🖥️ CLIENT SIDE
      │    ├── mod.rs         # Client module aggregator
      │    ├── cli.rs         # UI Core: Ratatui rendering, key event handling, state machine (AppMode)
      │    ├── auth.rs        # Auth logic, RSA key pair generation, active account management
      │    ├── crypto.rs      # Hybrid crypto core (RSA key encapsulation + AES-256-GCM text encryption)
      │    ├── network.rs     # Async TCP/WS client (connection management, socket polling, background pings)
      │    ├── storage.rs     # Local SQLite storage (message history, peer caching, server address index)
      │    └── messages.rs    # Message parsing and formatting layout engine
      │
      └── server/             # 📡 SERVER SIDE
           ├── main.rs        # Server entry point binary (cargo run --bin cult-server)
           ├── network.rs     # TCP/WS server: port binding, incoming connection handling loop
           ├── auth.rs        # Registration validation (PublicKey + Username identity mapping)
           ├── registry.rs    # In-memory connected peers registry (live socket index and online states)
           └── queue.rs       # Offline message queue store (store-and-forward edge cache for offline peers)
```
Compilation and Local Setup
Prerequisites

Ensure you have the Rust toolchain installed (Rustc 1.75+ recommended):
Bash
```
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```
Building the Project

Clone the repository and build the production-ready optimized binaries:
Bash
```
git clone https://github.com/ideikaPIX/cult-net.git
```
```
cd cult-net
```
```
cargo build --release
```

The compiled binaries will be available in the ./target/release/ directory.
Server Deployment Guide

This guide covers deploying the cult-server daemon to a production Linux environment (Ubuntu 24.04 LTS / Debian).
Step 1: Environment & Firewall Preparation

Secure your environment by restricting incoming traffic. The CULT.NET server operates on a custom TCP port (e.g., 8080).

Ensure your UFW (Uncomplicated Firewall) rules allow SSH and the chat server communication port:
Bash

```
sudo ufw default deny incoming
```
```
sudo ufw default allow outgoing
```
```
sudo ufw allow ssh
```
```
sudo ufw allow 8080/tcp
```
```
sudo ufw enable
```

Step 2: Binary Deployment


 Move the compiled release binary to a global binary directory:
```
sudo cp target/release/cult-server /usr/local/bin/cult-server
```
```
sudo chmod +x /usr/local/bin/cult-server
``` 
 Create a dedicated system user to isolate system privileges:
```
sudo useradd -m -s /usr/sbin/nologin cult
```
Step 3: Configuring the Systemd Daemon

To ensure the server starts automatically on boot, restarts on crashes, and logs errors properly, configure a systemd service unit.

Create the service file:
Bash

```
sudo nano /etc/systemd/system/cult-server.service
```

Paste the following system configuration:
Ini, TOML
```
[Unit]
Description=CULT.NET Secure Messenger Server Daemon
After=network.target

[Service]
Type=simple
User=cult
WorkingDirectory=/home/cult
ExecStart=/usr/local/bin/cult-server --host 0.0.0.0 --port 8080
Restart=always
RestartSec=5
StandardOutput=journal
StandardError=journal
SyslogIdentifier=cult-server

[Install]
WantedBy=multi-user.target
```
Step 4: Activating the Service

Reload systemd, enable the service on system startup, and launch the server daemon:
Bash
```
sudo systemctl daemon-reload
```
```
sudo systemctl enable cult-server.service
```
```
sudo systemctl start cult-server.service
```
Step 5: Service Monitoring & Maintenance

To check the operational status of your server instance:
Bash
```
sudo systemctl status cult-server.service
```
To view live application logs and packet routing tracking in real time:
Bash
```
sudo journalctl -u cult-server.service -f -n 100
```
TUI Quick Shortcut Sheet:
    
        [← / →  ↑ / ↓] : arrows control 
        [↵] : Enter
        [p] : Peers
        [d] : Delete
        [s] : Switch (accounts)
        [q] : quit
        [Esc] - Escape
