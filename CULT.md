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

## Analysis & Improvement Suggestions

### Observations
- The project clearly separates concerns between client and server modules.
- Uses `tokio` for async runtime.
- Server-side imports `shared.rs` using `#[path]`, which is a common but slightly brittle pattern in Rust workspaces/crates.

### Suggestions for Improvement
1. **Workspace Configuration:** Instead of relying on `#[path]`, convert the project into a proper Cargo workspace with a shared `lib` crate. This will allow cleaner imports and better dependency management.
2. **Error Handling:** Centralize error types using a library like `thiserror` or `anyhow` to replace the generic `Box<dyn Error>`.
3. **Testing:** Implement integration tests to verify the communication flow between client and server modules.
4. **Configuration:** Externalize configuration (IP, ports, keys) into a TOML or YAML file rather than hardcoding them.
5. **Tracing/Logging:** Implement formal logging (`tracing` crate) to replace standard `println!` statements for better production observability.
