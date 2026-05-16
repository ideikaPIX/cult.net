# CULT.NET Project Documentation

## Project Overview
CULT.NET is a Rust-based project implementing a distributed messaging or network service architecture. It follows a client-server model, utilizing `tokio` for asynchronous networking. The system includes features for authentication, cryptography, message handling, and server-side state management (registry and queues).

## Directory Structure
- `src/`
  - `main.rs`: Entry point for the client-side CLI.
  - `shared.rs`: Shared utilities or data models between client and server.
  - `client/`: Client-side logic.
    - `auth.rs`: Client-side authentication logic.
    - `cli.rs`: CLI interface implementation.
    - `crypto.rs`: Cryptographic operations.
    - `messages.rs`: Message definitions and serialization/deserialization.
    - `network.rs`: Networking layer for clients.
    - `storage.rs`: Local storage management.
  - `server/`: Server-side logic.
    - `main.rs`: Server entry point.
    - `auth.rs`: Server-side authentication logic.
    - `network.rs`: Server networking and request handling.
    - `queue.rs`: Server-side queue management.
    - `registry.rs`: Server-side client/resource registry.
