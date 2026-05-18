# CULT.NET

🔐 **CULT.NET** is an ultra-compact, high-security, two-pane terminal messenger written in Rust. It features an end-to-end encrypted hybrid cryptographic core, an asynchronous networking engine with automatic failovers, and a highly optimized monocolor TUI dashboard tailored specifically for narrow viewports, system administrators, and power users (WSL, Termux, Linux consoles).

---

## Architecture Overview

### 1. Hybrid Cryptography Core
To ensure maximum performance without compromising security, CULT.NET uses a dual-layer hybrid encryption pipeline:
* **Asymmetric Layer (RSA-2048):** Used for initial key exchange and identity verification between peers.
* **Symmetric Layer (AES-256-GCM):** Once a secure session is initiated, messages are packed with a unique `nonce` and encrypted via hardware-accelerated AES.
* **Fallback Protocol:** If an incoming payload fails to decrypt via AES (e.g., due to session desynchronization), the client automatically triggers an RSA decryption fallback to process identity payloads without dropping the connection.

### 2. Network Gateway & Persistence
* **Resilient Connection Handling:** The client utilizes a 3-attempt connection loop before prompting a fallback state.
* **Smart Offline Mode:** If the server is unreachable, users can drop directly into local-only mode (`[↵] Offline mode`) to browse historical logs.
* **Asynchronous Background Pinger:** An active background scanner pings configured peers every 20 seconds even before entering the active peer view (`AppMode::Peers`), ensuring live status metrics immediately upon UI initialization.
* **Local Storage:** Message history and credential indexing are securely maintained via a local SQLite database infrastructure.

### 3. Streamlined TUI Design
* **Dual-Pane Interface:** Fixed 28-column sidebar for stateful contact navigation, alongside a dynamic chat grid.
* **Dynamic Hint Line:** The bottom navigation bar mutates in real-time, showing *only* context-relevant shortcuts to save valuable screen space.
* **Truncation & Overlay:** Long peer handles are automatically clipped (`..`). When highlighted, the viewport bypasses layout boundaries to render the address as a layout overlay.
* **Interactive Scroll:** Incorporates a high-contrast yellow triple-arrow indicator (`▲▲▲`) in the lower-left corner of the message pad when viewing older historical segments.

---

## Project Structure
```
├── Cargo.toml
├── src
│   ├── main.rs          # Application entry point
│   ├── server           # Server architecture & packet routing
│   │   └── mod.rs
│   └── client           # Client logic and UI rendering
│       ├── mod.rs
│       ├── cli.rs       # Ratatui TUI Layout engine & dynamic views
│       └── network.rs   # Async networking & crypto primitives
```
Compilation and Local Setup
Prerequisites

Ensure you have the Rust toolchain installed (Rustc 1.75+ recommended):
Bash
```
curl --proto '=https' --tlsv1.2 -sSf [https://sh.rustup.rs](https://sh.rustup.rs) | sh
```
Building the Project

Clone the repository and build the production-ready optimized binaries:
Bash
```
git clone [https://github.com/ideikaPIX/cult-net.git](https://github.com/ideikaPIX/cult-net.git)
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
TUI Quick Shortcut Sheet

Depending on your active mode, the context bar will dynamically shift to show short individual key handles:

    Authentication / Login Screen:

        [← / →] : Switch between local profiles / accounts.

        [↵] : Validate selected cryptographic keys.

    Peers Panel (Sidebar Mode):

        [p] : Toggle peer navigation focus.

        [d] : Remove selected peer identity from SQLite records.

        [s] : Enter global account/auth switcher interface.

        [q] : Gracefully exit terminal application framework.

    Chat Panel (Active Session):

        [↵] : Encrypt input buffer via AES-256-GCM and send packet.

        [Esc] : Lose chat focus and slide back to peer selection panel safely.
