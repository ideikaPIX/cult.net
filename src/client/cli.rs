use crossterm::{
    event::{self, Event, KeyCode, KeyModifiers},
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    ExecutableCommand,
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    widgets::{Block, Borders, List, ListItem, Paragraph, ListState},
    style::{Style, Color, Modifier},
    text::{Line, Span},
    Frame, Terminal,
};
use std::io::stdout;
use crate::client::{auth, network, storage};
use crate::shared::{ClientMessage, ServerResponse};
use std::time::{Duration, Instant};
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::sync::mpsc;
use std::collections::HashMap;

fn to_superscript(num: usize) -> String {
    num.to_string()
        .chars()
        .map(|c| match c {
            '0' => '⁰', '1' => '¹', '2' => '²', '3' => '³', '4' => '⁴',
            '5' => '⁵', '6' => '⁶', '7' => '⁷', '8' => '⁸', '9' => '⁹',
            _ => c,
        })
        .collect()
}

#[derive(PartialEq, Clone, Copy)]
enum AppMode {
    EnterServerIp,
    Auth,
    AuthSwitch,
    DeleteAccount,
    Main,
    Peers,
    Chat,
    Switch,
    AddContact,
    DeleteContact,
    ShowKey,
}

pub async fn run_cli() -> Result<(), Box<dyn std::error::Error>> {
    let mut stdout = stdout();
    enable_raw_mode()?;
    stdout.execute(EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    terminal.clear()?;

    let res = run_app(&mut terminal).await;

    disable_raw_mode()?;
    terminal.backend_mut().execute(LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    if let Err(err) = res {
        println!("{:?}", err)
    }

    Ok(())
}

async fn run_app(
    terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut mode = AppMode::EnterServerIp;
    let mut input_buffer = String::new();
    let mut active_peer = String::new();
    let mut contacts = storage::load_contacts().unwrap_or_default().contacts;
    let mut accounts = storage::load_accounts().unwrap_or_default().accounts;
    let mut message_history: Vec<(String, String)> = Vec::new();
    let mut delete_confirm: bool = false;
    let mut last_mode = mode;
    let mut scroll_offset: u16 = 0;
    let mut is_autoscroll: bool = true;
    let mut peer_status: Option<(bool, Option<String>)> = None;

    let mut server_history = storage::load_server_history().unwrap_or_default().ips;
    let mut server_history_state = ListState::default();
    
    let mut switch_acc_state = ListState::default();
    let mut peers_state = ListState::default();
    let mut delete_contact_state = ListState::default();
    let mut delete_account_state = ListState::default();

    let mut net_client: Option<network::NetworkClient> = None;
    let mut account: Option<storage::Account> = None;
    let mut receiver: Option<Arc<Mutex<mpsc::UnboundedReceiver<ServerResponse>>>> = None;
    let mut connection_error = String::new();

    let mut active_acc = auth::get_active_account().unwrap_or(None);
    let mut delete_parent_mode: Option<AppMode> = None;
    let mut current_server_ip: Option<String> = None;
    
    let mut last_poll = Instant::now();
    let poll_interval = Duration::from_secs(5);

    let mut unread_counts: HashMap<String, usize> = HashMap::new();
    let mut peer_statuses: HashMap<String, (bool, Option<String>)> = HashMap::new();

    let (connect_tx, mut connect_rx) = mpsc::unbounded_channel::<(Option<network::NetworkClient>, String, Option<&'static str>)>();
    let (key_tx, mut key_rx) = mpsc::unbounded_channel::<storage::Account>();

    let spinner_frames = vec!["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
    let mut current_frame = 0;
    let mut is_connecting = false;
    let mut connecting_to: Option<String> = None;
    let mut is_generating_keys = false;

    let mut ephemeral_aes_keys: std::collections::HashMap<String, [u8; 32]> = std::collections::HashMap::new();

    loop {
        if is_connecting || is_generating_keys {
            current_frame = (current_frame + 1) % spinner_frames.len();
        }

        if let Ok((client_opt, connected_ip, err_msg)) = connect_rx.try_recv() {
            is_connecting = false;
            connecting_to = None;
            if let Some(actual_client) = client_opt {
                receiver = Some(actual_client.receiver.clone());
                net_client = Some(actual_client);
                current_server_ip = Some(connected_ip);
                accounts = storage::load_accounts().unwrap_or_default().accounts;
                mode = AppMode::Auth;
            } else {
                connection_error = format!(" {}", err_msg.unwrap_or("Connection failed"));
            }
        }

        if let Ok(acc) = key_rx.try_recv() {
            is_generating_keys = false;
            if let Some(c) = net_client.as_mut() {
                let msg = ClientMessage::Register {
                    username: acc.username.clone(),
                    public_key: acc.public_key.clone(),
                };
                let _ = c.sender.send(msg);
            }
            account = Some(acc);
            input_buffer.clear();
            mode = AppMode::ShowKey;
        }

        if account.is_some() && last_poll.elapsed() >= poll_interval {
            if let Some(client) = net_client.as_mut() {
                for contact in &contacts {
                    let _ = client.sender.send(ClientMessage::CheckStatus { target: contact.full_address.clone() });
                }
            }
            last_poll = Instant::now();
        }

        if let Some(ref r) = receiver {
            if let Ok(mut lock) = r.try_lock() {
                while let Ok(resp) = lock.try_recv() {
                    match resp {
                        ServerResponse::IncomingMessage { from, encrypted_content, timestamp } => {
                            if let Some(acc) = account.as_ref() {
                                if let Ok(payload) = serde_json::from_str::<crate::client::crypto::InnerPayload>(&encrypted_content) {
                                    match payload {
                                        crate::client::crypto::InnerPayload::KeyInit { encrypted_aes_key } => {
                                            eprintln!("[DEBUG] Received KeyInit from {} for session establishment", from);
                                            if let Ok(enc_str) = String::from_utf8(encrypted_aes_key) {
                                                if let Ok(dec_b64) = crate::client::crypto::decrypt(&enc_str, &acc.private_key) {
                                                    use base64::{Engine as _, engine::general_purpose};
                                                    if let Ok(key_vec) = general_purpose::STANDARD.decode(dec_b64) {
                                                        if key_vec.len() == 32 {
                                                            let mut arr = [0u8; 32];
                                                            arr.copy_from_slice(&key_vec);
                                                            ephemeral_aes_keys.insert(from.clone(), arr);
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                        crate::client::crypto::InnerPayload::SecureText { ciphertext, nonce } => {
                                            let dec_content = if let Some(aes_key) = ephemeral_aes_keys.get(&from) {
                                                crate::client::crypto::aes_decrypt(&ciphertext, aes_key, &nonce)
                                                    .unwrap_or_else(|_| " [AES Decryption failed]".to_string())
                                            } else {
                                                " [Missing AES key]".to_string()
                                            };

                                            if active_peer == from && mode == AppMode::Chat {
                                                message_history.push((from.clone(), dec_content.clone()));
                                                is_autoscroll = true;
                                            } else {
                                                *unread_counts.entry(from.clone()).or_insert(0) += 1;
                                            }
                                            
                                            if let Ok(conn) = storage::get_chat_db(&from) {
                                                let _ = conn.execute(
                                                    "INSERT INTO messages (timestamp, sender, content, status, is_yours) VALUES (?1, ?2, ?3, ?4, ?5)",
                                                    (timestamp, from.clone(), dec_content, "received", false),
                                                );
                                            }
                                        }
                                    }
                                }
                            }
                        },
                        ServerResponse::KeyResponse { public_key, online_status: _ } => {
                            eprintln!("[DEBUG] Received public key for {}", active_peer);
                            if let Some(contact) = contacts.iter_mut().find(|c| c.full_address == active_peer) {
                                contact.public_key = public_key.clone();
                                let mut data = storage::load_contacts().unwrap_or_default();
                                if let Some(c) = data.contacts.iter_mut().find(|c| c.full_address == active_peer) {
                                    c.public_key = public_key.clone();
                                    let _ = storage::save_contacts(&data);
                                }
                            }

                            if !public_key.is_empty() && !ephemeral_aes_keys.contains_key(&active_peer) {
                                use rand::Rng;
                                use base64::{Engine as _, engine::general_purpose};
                                
                                let mut new_key = [0u8; 32];
                                rand::thread_rng().fill(&mut new_key);

                                let new_key_b64 = general_purpose::STANDARD.encode(new_key);
                                if let Ok(encrypted_aes_str) = crate::client::crypto::encrypt(&new_key_b64, &public_key) {
                                    let payload = crate::client::crypto::InnerPayload::KeyInit { 
                                        encrypted_aes_key: encrypted_aes_str.into_bytes() 
                                    };
                                    if let Ok(json_str) = serde_json::to_string(&payload) {
                                        if let Some(client) = net_client.as_mut() {
                                            if let Some(acc) = account.as_ref() {
                                                eprintln!("[DEBUG] Sending KeyInit to establishment secure channel with {}", active_peer);
                                                let msg = ClientMessage::SendMessage {
                                                    from: acc.full_address.clone(),
                                                    to: active_peer.clone(),
                                                    encrypted_content: json_str,
                                                    timestamp: chrono::Utc::now().to_rfc3339(),
                                                };
                                                let _ = client.sender.send(msg);
                                            }
                                        }
                                        ephemeral_aes_keys.insert(active_peer.clone(), new_key);
                                    }
                                }
                            }
                        },
                        ServerResponse::StatusResponse { target, online, last_seen } => {
                            if target == active_peer {
                                peer_status = Some((online, last_seen.clone()));
                            }
                            peer_statuses.insert(target, (online, last_seen));
                        },
                        ServerResponse::Error { message } => {
                            eprintln!("[DEBUG] Server error: {}", message);
                        },
                        _ => {}
                    }
                }
            }
        }

        if mode != last_mode {
            terminal.clear()?;
            last_mode = mode;
        }

        terminal.draw(|f| ui(
            f, 
            &mode, 
            account.as_ref(), 
            net_client.is_some(), 
            &input_buffer, 
            &active_peer, 
            &contacts, 
            &accounts, 
            &message_history,
            &mut delete_contact_state,
            delete_confirm,
            &connection_error,
            active_acc.as_ref().map(|a| a.full_address.clone()),
            &mut scroll_offset,
            &mut is_autoscroll,
            &peer_status,
            &server_history,
            &mut server_history_state,
            &mut switch_acc_state,
            &mut peers_state,
            &mut delete_account_state,
            &unread_counts,
            &peer_statuses,
            is_connecting,
            &connecting_to,
            is_generating_keys,
            current_frame,
            &spinner_frames,
        ))?;

        if crossterm::event::poll(Duration::from_millis(60))? {
            if let Event::Key(key) = event::read()? {
                if key.kind != event::KeyEventKind::Press {
                    continue;
                }
                
                // Ensure default selection for lists
                if mode == AppMode::EnterServerIp && server_history_state.selected().is_none() && !server_history.is_empty() {
                    server_history_state.select(Some(0));
                } else if mode == AppMode::Peers && peers_state.selected().is_none() && !contacts.is_empty() {
                    peers_state.select(Some(0));
                }

                if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
                    return Ok(());
                }

                match mode {
                    AppMode::EnterServerIp => match key.code {
                        KeyCode::Char(c) => input_buffer.push(c),
                        KeyCode::Backspace => { input_buffer.pop(); },
                        KeyCode::Up => {
                            input_buffer.clear();
                            if !server_history.is_empty() {
                                let i = match server_history_state.selected() {
                                    Some(i) => if i == 0 { 0 } else { i - 1 },
                                    None => 0,
                                };
                                server_history_state.select(Some(i));
                            }
                        }
                        KeyCode::Down => {
                            input_buffer.clear();
                            if !server_history.is_empty() {
                                let i = match server_history_state.selected() {
                                    Some(i) => if i >= server_history.len() - 1 { server_history.len() - 1 } else { i + 1 },
                                    None => 0,
                                };
                                server_history_state.select(Some(i));
                            }
                        }
                        KeyCode::Enter => {
                            let ip = if !input_buffer.is_empty() {
                                input_buffer.trim().to_string()
                            } else if let Some(i) = server_history_state.selected() {
                                server_history.get(i).cloned().unwrap_or_default()
                            } else {
                                "".to_string()
                            };

                            if !ip.is_empty() {
                                server_history.retain(|x| x != &ip);
                                server_history.insert(0, ip.clone());
                                let _ = storage::save_server_history(&storage::ServerHistory { ips: server_history.clone() });
                                if !server_history.is_empty() {
                                    server_history_state.select(Some(0));
                                }

                                connection_error.clear();
                                let ip_clone = ip.clone();
                                let tx = connect_tx.clone();
                                input_buffer.clear();
                                is_connecting = true;
                                connecting_to = Some(ip_clone.clone());

                                tokio::spawn(async move {
                                    let connected_client = match tokio::time::timeout(Duration::from_secs(3), network::connect(&ip_clone)).await {
                                        Ok(Ok(client)) => Ok(client),
                                        Ok(Err(_)) => Err("Failed to connect"),
                                        Err(_) => Err("Timeout"),
                                    };
                                    let _ = tx.send((connected_client.as_ref().ok().cloned(), ip_clone, connected_client.err()));
                                });
                            }
                        }
                        KeyCode::Esc => return Ok(()),
                        _ => {}
                    },
                    AppMode::Auth => match key.code {
                        KeyCode::Right => {
                            accounts = storage::load_accounts().unwrap_or_default().accounts;
                            if !accounts.is_empty() {
                                switch_acc_state.select(Some(0));
                                mode = AppMode::AuthSwitch;
                            }
                        }
                        KeyCode::Char('d') | KeyCode::Delete => {
                            accounts = storage::load_accounts().unwrap_or_default().accounts;
                            if !accounts.is_empty() {
                                delete_account_state.select(Some(0));
                                mode = AppMode::DeleteAccount;
                            }
                        }
                        KeyCode::Char(c) => input_buffer.push(c),
                        KeyCode::Backspace => { input_buffer.pop(); },
                        KeyCode::Enter => {
                            let username = input_buffer.trim().to_string();
                            if username.is_empty() && active_acc.is_some() {
                                let acc = active_acc.clone().unwrap();
                                if let Some(c) = net_client.as_mut() {
                                    let msg = ClientMessage::Register {
                                        username: acc.username.clone(),
                                        public_key: acc.public_key.clone(),
                                    };
                                    let _ = c.sender.send(msg);
                                }
                                account = Some(acc);
                                input_buffer.clear();
                                mode = AppMode::Main;
                            } else {
                                let actual_username = if username.is_empty() { "anonymous".to_string() } else { username };
                                is_generating_keys = true;
                                let tx = key_tx.clone();
                                tokio::task::spawn_blocking(move || {
                                    if let Ok(acc) = auth::login(&actual_username, true) {
                                        let _ = tx.send(acc);
                                    }
                                });
                            }
                        }
                        KeyCode::Esc => return Ok(()),
                        _ => {}
                    },
                    AppMode::DeleteAccount => match key.code {
                        KeyCode::Esc | KeyCode::Left | KeyCode::Char('q') | KeyCode::Char('b') => {
                            if delete_parent_mode == Some(AppMode::Switch) {
                                if let Some(current_idx) = delete_account_state.selected() {
                                    switch_acc_state.select(Some(current_idx));
                                }
                            }
                            mode = delete_parent_mode.unwrap_or(AppMode::AuthSwitch);
                        }
                        KeyCode::Up => {
                            if !accounts.is_empty() {
                                let i = match delete_account_state.selected() {
                                    Some(i) => if i == 0 { 0 } else { i - 1 },
                                    None => 0,
                                };
                                delete_account_state.select(Some(i));
                            }
                        }
                        KeyCode::Down => {
                            if !accounts.is_empty() {
                                let i = match delete_account_state.selected() {
                                    Some(i) => if i >= accounts.len() - 1 { accounts.len() - 1 } else { i + 1 },
                                    None => 0,
                                };
                                delete_account_state.select(Some(i));
                            }
                        }
                        KeyCode::Enter => {
                            if let Some(idx) = delete_account_state.selected() {
                                let removed = accounts.remove(idx);
                                let _ = storage::save_accounts(&storage::AccountsData { accounts: accounts.clone() });
                                if active_acc.as_ref().map(|a| a.full_address == removed.full_address).unwrap_or(false) {
                                    active_acc = None;
                                }

                                let next_idx = if accounts.is_empty() {
                                    None
                                } else if idx >= accounts.len() {
                                    Some(accounts.len() - 1)
                                } else {
                                    Some(idx)
                                };

                                if delete_parent_mode == Some(AppMode::Switch) {
                                    switch_acc_state.select(next_idx);
                                }
                                delete_account_state.select(next_idx);
                                mode = delete_parent_mode.unwrap_or(AppMode::AuthSwitch);
                            }
                        }
                        _ => {}
                    },
                    AppMode::AuthSwitch => match key.code {
                        KeyCode::Esc | KeyCode::Left => mode = AppMode::Auth,
                        KeyCode::Char('d') | KeyCode::Delete => {
                            if !accounts.is_empty() {
                                delete_account_state.select(switch_acc_state.selected());
                                mode = AppMode::DeleteAccount;
                            }
                        }
                        KeyCode::Up => {
                            let i = match switch_acc_state.selected() {
                                Some(i) => if i == 0 { 0 } else { i - 1 },
                                None => 0,
                            };
                            switch_acc_state.select(Some(i));
                        }
                        KeyCode::Down => {
                            let i = match switch_acc_state.selected() {
                                Some(i) => if i >= accounts.len().saturating_sub(1) { accounts.len().saturating_sub(1) } else { i + 1 },
                                None => 0,
                            };
                            switch_acc_state.select(Some(i));
                        }
                        KeyCode::Enter => {
                            if let Some(i) = switch_acc_state.selected() {
                                if let Some(selected_acc) = accounts.get(i).cloned() {
                                    if let Some(ref mut client) = net_client {
                                        let _ = client.sender.send(ClientMessage::Disconnect);
                                        std::thread::sleep(std::time::Duration::from_millis(50));
                                    }

                                    if auth::switch_account(&selected_acc.full_address).unwrap_or(false) {
                                        active_acc = Some(selected_acc);
                                        input_buffer.clear();
                                    }

                                    if let Some(ref ip) = current_server_ip {
                                        net_client = None;
                                        receiver = None;
                                        
                                        let ip_clone = ip.clone();
                                        let tx = connect_tx.clone();
                                        connection_error = "Switching session...".to_string();
                                        
                                        tokio::spawn(async move {
                                            let connected_client = match tokio::time::timeout(Duration::from_secs(3), network::connect(&ip_clone)).await {
                                                Ok(Ok(client)) => Ok(client),
                                                Ok(Err(_)) => Err("Failed to connect"),
                                                Err(_) => Err("Timeout"),
                                            };
                                            let _ = tx.send((connected_client.as_ref().ok().cloned(), ip_clone, connected_client.err()));
                                        });                                    } else {
                                        mode = AppMode::Auth;
                                    }
                                }
                            }
                        }
                        _ => {}
                    },
                    AppMode::Main => match key.code {
                        KeyCode::Char('q') => return Ok(()),
                        KeyCode::Char('p') => {
                            contacts = storage::load_contacts().unwrap_or_default().contacts;
                            if let Some(client) = net_client.as_mut() {
                                for contact in &contacts {
                                    let _ = client.sender.send(ClientMessage::CheckStatus { target: contact.full_address.clone() });
                                }
                            }
                            last_poll = Instant::now();
                            mode = AppMode::Peers;
                        }
                        KeyCode::Char('s') | KeyCode::Char('S') => {
                            accounts = storage::load_accounts().unwrap_or_default().accounts;
                            if !accounts.is_empty() {
                                switch_acc_state.select(Some(0));
                            }
                            mode = AppMode::Switch;
                        }
                        _ => {}
                    },
                    AppMode::Peers => match key.code {
                        KeyCode::Char('b') | KeyCode::Esc => mode = AppMode::Main,
                        KeyCode::Char('a') => {
                            input_buffer.clear();
                            mode = AppMode::AddContact;
                        }
                        KeyCode::Char('d') => {
                            if !contacts.is_empty() {
                                delete_contact_state.select(peers_state.selected());
                                delete_confirm = false;
                                delete_parent_mode = Some(AppMode::Peers);
                                mode = AppMode::DeleteContact;
                            }
                        }
                        KeyCode::Char('s') | KeyCode::Char('S') => {
                            accounts = storage::load_accounts().unwrap_or_default().accounts;
                            if !accounts.is_empty() {
                                switch_acc_state.select(Some(0));
                            }
                            mode = AppMode::Switch;
                        }
                        KeyCode::Up => {
                            if !contacts.is_empty() {
                                let i = match peers_state.selected() {
                                    Some(i) => if i == 0 { 0 } else { i - 1 },
                                    None => 0,
                                };
                                peers_state.select(Some(i));
                            }
                        }
                        KeyCode::Down => {
                            if !contacts.is_empty() {
                                let i = match peers_state.selected() {
                                    Some(i) => if i >= contacts.len() - 1 { contacts.len() - 1 } else { i + 1 },
                                    None => 0,
                                };
                                peers_state.select(Some(i));
                            }
                        }
                        KeyCode::Enter => {
                            if let Some(idx) = peers_state.selected() {
                                if let Some(contact) = contacts.get(idx) {
                                    active_peer = contact.full_address.clone();
                                    unread_counts.insert(active_peer.clone(), 0);
                                    
                                    if let Some(client) = net_client.as_mut() {
                                        eprintln!("[DEBUG] Requesting public key for {}", active_peer);
                                        let _ = client.sender.send(ClientMessage::GetPublicKey { target: active_peer.clone() });
                                    }
                                    
                                    if let Some(client) = net_client.as_mut() {
                                        let _ = client.sender.send(ClientMessage::CheckStatus { target: active_peer.clone() });
                                        last_poll = Instant::now();
                                    }
                                    peer_status = peer_statuses.get(&active_peer).cloned();
                                    
                                    message_history.clear();
                                    if let Ok(conn) = storage::get_chat_db(&active_peer) {
                                        if let Ok(mut stmt) = conn.prepare("SELECT sender, content FROM messages ORDER BY id ASC LIMIT 50") {
                                            let msg_iter = stmt.query_map([], |row| {
                                                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
                                            });
                                            if let Ok(msgs) = msg_iter {
                                                for m in msgs {
                                                    if let Ok(m) = m { message_history.push(m); }
                                                }
                                            }
                                        }
                                    }
                                    input_buffer.clear();
                                    mode = AppMode::Chat;
                                }
                            }
                        }
                        _ => {}
                    },
                    AppMode::Chat => match key.code {
                        KeyCode::Esc => {
                            if let Some(client) = net_client.as_mut() {
                                for contact in &contacts {
                                    let _ = client.sender.send(ClientMessage::CheckStatus { target: contact.full_address.clone() });
                                }
                            }
                            last_poll = Instant::now();
                            mode = AppMode::Peers;
                        },
                        KeyCode::Up | KeyCode::PageUp => {
                            if scroll_offset > 0 {
                                scroll_offset = scroll_offset.saturating_sub(1);
                            }
                            is_autoscroll = false;
                        }
                        KeyCode::Down | KeyCode::PageDown => {
                            scroll_offset = scroll_offset.saturating_add(1);
                        }
                        KeyCode::Char(c) => {
                            input_buffer.push(c);
                        }
                        KeyCode::Backspace => {
                            input_buffer.pop();
                        }
                        KeyCode::Enter => {
                            let text = input_buffer.trim().to_string();
                            if !text.is_empty() {
                                is_autoscroll = true;
                                let peer_pub_key = contacts.iter()
                                    .find(|c| c.full_address == active_peer)
                                    .map(|c| c.public_key.clone())
                                    .unwrap_or_default();
                                
                                if peer_pub_key.is_empty() {
                                    let db_content = format!("⚠️UNENCRYPTED: {}", text);
                                    if let Some(client) = net_client.as_mut() {
                                        if let Some(acc) = account.as_ref() {
                                            let msg = ClientMessage::SendMessage {
                                                from: acc.full_address.clone(),
                                                to: active_peer.clone(),
                                                encrypted_content: text.clone(),
                                                timestamp: chrono::Utc::now().to_rfc3339(),
                                            };
                                            let _ = client.sender.send(msg);
                                        }
                                    }
                                    if let Ok(conn) = storage::get_chat_db(&active_peer) {
                                        if let Some(acc) = account.as_ref() {
                                            let ts = chrono::Utc::now().to_rfc3339();
                                            let _ = conn.execute(
                                                "INSERT INTO messages (timestamp, sender, content, status, is_yours) VALUES (?1, ?2, ?3, ?4, ?5)",
                                                (ts, &acc.full_address, &db_content, "pending", true),
                                            );
                                            message_history.push((acc.full_address.clone(), db_content));
                                        }
                                    }
                                } else {
                                    if !ephemeral_aes_keys.contains_key(&active_peer) {
                                        use rand::Rng;
                                        use base64::{Engine as _, engine::general_purpose};
                                        
                                        let mut new_key = [0u8; 32];
                                        rand::thread_rng().fill(&mut new_key);

                                        let new_key_b64 = general_purpose::STANDARD.encode(new_key);
                                        if let Ok(encrypted_aes_str) = crate::client::crypto::encrypt(&new_key_b64, &peer_pub_key) {
                                            let payload = crate::client::crypto::InnerPayload::KeyInit { 
                                                encrypted_aes_key: encrypted_aes_str.into_bytes() 
                                            };
                                            if let Ok(json_str) = serde_json::to_string(&payload) {
                                                if let Some(client) = net_client.as_mut() {
                                                    if let Some(acc) = account.as_ref() {
                                                        let msg = ClientMessage::SendMessage {
                                                            from: acc.full_address.clone(),
                                                            to: active_peer.clone(),
                                                            encrypted_content: json_str,
                                                            timestamp: chrono::Utc::now().to_rfc3339(),
                                                        };
                                                        let _ = client.sender.send(msg);
                                                    }
                                                }
                                                ephemeral_aes_keys.insert(active_peer.clone(), new_key);
                                            }
                                        }
                                    }

                                    if let Some(aes_key) = ephemeral_aes_keys.get(&active_peer) {
                                        if let Ok((ciphertext, nonce)) = crate::client::crypto::aes_encrypt(&text, aes_key) {
                                            let payload = crate::client::crypto::InnerPayload::SecureText { ciphertext, nonce };
                                            if let Ok(json_str) = serde_json::to_string(&payload) {
                                                if let Some(client) = net_client.as_mut() {
                                                    if let Some(acc) = account.as_ref() {
                                                        let msg = ClientMessage::SendMessage {
                                                            from: acc.full_address.clone(),
                                                            to: active_peer.clone(),
                                                            encrypted_content: json_str,
                                                            timestamp: chrono::Utc::now().to_rfc3339(),
                                                        };
                                                        let _ = client.sender.send(msg);
                                                    }
                                                }
                                                
                                                if let Ok(conn) = storage::get_chat_db(&active_peer) {
                                                    if let Some(acc) = account.as_ref() {
                                                        let ts = chrono::Utc::now().to_rfc3339();
                                                        let _ = conn.execute(
                                                            "INSERT INTO messages (timestamp, sender, content, status, is_yours) VALUES (?1, ?2, ?3, ?4, ?5)",
                                                            (ts, &acc.full_address, &text, "pending", true),
                                                        );
                                                        message_history.push((acc.full_address.clone(), text.clone()));
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                            input_buffer.clear();
                        }
                        _ => {}
                    },
                    AppMode::AddContact => match key.code {
                        KeyCode::Esc => {
                            input_buffer.clear();
                            mode = AppMode::Peers;
                        }
                        KeyCode::Char(c) => input_buffer.push(c),
                        KeyCode::Backspace => { input_buffer.pop(); }
                        KeyCode::Enter => {
                            let peer = input_buffer.trim().to_string();
                            if !peer.is_empty() {
                                let is_valid = if let Some(rest) = peer.strip_suffix("@cult.net") {
                                    let parts: Vec<&str> = rest.split('#').collect();
                                    parts.len() == 2 && !parts[0].is_empty() && parts[1].len() <= 4 && !parts[1].is_empty()
                                } else {
                                    false
                                };

                                if is_valid {
                                    let mut contacts_data = storage::load_contacts().unwrap_or_default();
                                    let hash_split: Vec<&str> = peer.split('#').collect();
                                    let username = hash_split[0].to_string();
                                    let peer_id = hash_split[1].split('@').next().unwrap().to_string();
                                    
                                    let contact = storage::Contact {
                                        username,
                                        peer_id,
                                        full_address: peer.clone(),
                                        public_key: "".to_string(),
                                        added_at: chrono::Utc::now().to_rfc3339(),
                                        last_message: None,
                                    };
                                    contacts_data.contacts.push(contact);
                                    let _ = storage::save_contacts(&contacts_data);
                                    contacts = contacts_data.contacts;
                                    if !contacts.is_empty() {
                                        peers_state.select(Some(0));
                                    }
                                    input_buffer.clear();
                                    mode = AppMode::Peers;
                                }
                            }
                        }
                        _ => {}
                    },
                    AppMode::DeleteContact => match key.code {
                        KeyCode::Esc | KeyCode::Left | KeyCode::Char('q') | KeyCode::Char('b') => {
                            if let Some(current_idx) = delete_contact_state.selected() {
                                peers_state.select(Some(current_idx));
                            }
                            mode = delete_parent_mode.unwrap_or(AppMode::Peers);
                        }
                        KeyCode::Up => {
                            if !contacts.is_empty() {
                                let i = match delete_contact_state.selected() {
                                    Some(i) => if i == 0 { 0 } else { i - 1 },
                                    None => 0,
                                };
                                delete_contact_state.select(Some(i));
                                delete_confirm = false;
                            }
                        }
                        KeyCode::Down => {
                            if !contacts.is_empty() {
                                let i = match delete_contact_state.selected() {
                                    Some(i) => if i >= contacts.len() - 1 { contacts.len() - 1 } else { i + 1 },
                                    None => 0,
                                };
                                delete_contact_state.select(Some(i));
                                delete_confirm = false;
                            }
                        }
                        KeyCode::Enter => {
                            if delete_confirm {
                                if let Some(idx) = delete_contact_state.selected() {
                                    let mut contacts_data = storage::load_contacts().unwrap_or_default();
                                    if idx < contacts_data.contacts.len() {
                                        contacts_data.contacts.remove(idx);
                                        let _ = storage::save_contacts(&contacts_data);
                                        contacts = contacts_data.contacts;
                                    }
                                    
                                    let next_idx = if contacts.is_empty() {
                                        None
                                    } else if idx >= contacts.len() {
                                        Some(contacts.len() - 1)
                                    } else {
                                        Some(idx)
                                    };

                                    peers_state.select(next_idx);
                                    delete_contact_state.select(next_idx);
                                    delete_confirm = false;
                                    mode = AppMode::Peers;
                                }
                            } else {
                                delete_confirm = true;
                            }
                        }
                        _ => {}
                    },
                    AppMode::ShowKey => match key.code {
                        KeyCode::Enter | KeyCode::Esc => mode = AppMode::Main,
                        _ => {}
                    },
                    AppMode::Switch => match key.code {
                        KeyCode::Esc | KeyCode::Char('b') => mode = AppMode::Main,
                        KeyCode::Char('d') => {
                            if !accounts.is_empty() {
                                delete_account_state.select(switch_acc_state.selected());
                                delete_parent_mode = Some(AppMode::Switch);
                                mode = AppMode::DeleteAccount;
                            }
                        }
                        KeyCode::Up => {
                            if !accounts.is_empty() {
                                let i = match switch_acc_state.selected() {
                                    Some(i) => if i == 0 { 0 } else { i - 1 },
                                    None => 0,
                                };
                                switch_acc_state.select(Some(i));
                            }
                        }
                        KeyCode::Down => {
                            if !accounts.is_empty() {
                                let i = match switch_acc_state.selected() {
                                    Some(i) => if i >= accounts.len() - 1 { accounts.len() - 1 } else { i + 1 },
                                    None => 0,
                                };
                                switch_acc_state.select(Some(i));
                            }
                        }
                        KeyCode::Enter => {
                            if let Some(idx) = switch_acc_state.selected() {
                                if let Some(selected_acc) = accounts.get(idx).cloned() {
                                    if let Some(ref mut client) = net_client {
                                        let _ = client.sender.send(ClientMessage::Disconnect);
                                        std::thread::sleep(std::time::Duration::from_millis(50));
                                    }

                                    if auth::switch_account(&selected_acc.full_address).unwrap_or(false) {
                                        active_acc = Some(selected_acc);
                                        input_buffer.clear();
                                    }

                                    if let Some(ref ip) = current_server_ip {
                                        net_client = None;
                                        receiver = None;
                                        
                                        let ip_clone = ip.clone();
                                        let tx = connect_tx.clone();
                                        connection_error = "Switching session...".to_string();
                                        
                                        tokio::spawn(async move {
                                            let connected_client = match tokio::time::timeout(Duration::from_secs(3), network::connect(&ip_clone)).await {
                                                Ok(Ok(client)) => Ok(client),
                                                Ok(Err(_)) => Err("Failed to connect"),
                                                Err(_) => Err("Timeout"),
                                            };
                                            let _ = tx.send((connected_client.as_ref().ok().cloned(), ip_clone, connected_client.err()));
                                        });                                    } else {
                                        mode = AppMode::Auth;
                                    }
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
    }
}

fn ui(
    f: &mut Frame,
    mode: &AppMode,
    account_opt: Option<&storage::Account>,
    is_online: bool,
    input_buffer: &str,
    active_peer: &str,
    contacts: &[storage::Contact],
    accounts: &[storage::Account],
    message_history: &[(String, String)],
    delete_contact_state: &mut ListState,
    delete_confirm: bool,
    connection_error: &str,
    active_acc_address: Option<String>,
    scroll_offset: &mut u16,
    is_autoscroll: &mut bool,
    peer_status: &Option<(bool, Option<String>)>,
    server_history: &[String],
    server_history_state: &mut ListState,
    switch_acc_state: &mut ListState,
    peers_state: &mut ListState,
    delete_account_state: &mut ListState,
    unread_counts: &HashMap<String, usize>,
    peer_statuses: &HashMap<String, (bool, Option<String>)>,
    is_connecting: bool,
    connecting_to: &Option<String>,
    is_generating_keys: bool,
    current_frame: usize,
    spinner_frames: &[&str],
) {
    f.render_widget(ratatui::widgets::Clear, f.area());
    let is_offline_mode = !is_online;

    match mode {
        AppMode::EnterServerIp => {
            if server_history_state.selected().is_none() && !server_history.is_empty() {
                server_history_state.select(Some(0));
            }
            let chunks = Layout::default().direction(Direction::Vertical).margin(1)
                .constraints([Constraint::Length(3), Constraint::Min(0)]).split(f.area());

            let text = if is_connecting {
                format!("{} Connecting to {}", spinner_frames[current_frame], connecting_to.as_deref().unwrap_or("?"))
            } else if !connection_error.is_empty() {
                connection_error.to_string()
            } else {
                input_buffer.to_string()
            };
            let title = if connection_error.is_empty() { "IP" } else { "Err" };
            let input = Paragraph::new(text).block(Block::default().title(title).borders(Borders::ALL));
            f.render_widget(input, chunks[0]);

            if !server_history.is_empty() {
                let items: Vec<ListItem> = server_history.iter().map(|ip| ListItem::new(ip.as_str())).collect();
                let history_list = List::new(items)
                    .block(Block::default().title("Recent").borders(Borders::ALL))
                    .highlight_style(Style::default().bg(Color::DarkGray).add_modifier(Modifier::BOLD))
                    .highlight_symbol(">>");
                f.render_stateful_widget(history_list, chunks[1], server_history_state);
            }
        }
        AppMode::Auth => {
            let chunks = Layout::default().direction(Direction::Vertical).margin(1)
                .constraints([Constraint::Length(3), Constraint::Min(0)]).split(f.area());
            let text = if is_generating_keys { format!("{} Gen RSA...", spinner_frames[current_frame]) } else { input_buffer.to_string() };
            
            let active_acc = accounts.iter().find(|a| a.is_active);
            let acc_display = active_acc
                .map(|a| format!("{}", a.full_address))
                .unwrap_or_else(|| "select acc".to_string());
                
            let title_text = format!("User: {} [↵] [→]", acc_display);
            let input = Paragraph::new(text).block(Block::default().title(title_text).borders(Borders::ALL));
            f.render_widget(input, chunks[0]);
        }
        AppMode::AuthSwitch | AppMode::Switch => {
            let chunks = Layout::default().direction(Direction::Vertical).margin(1)
                .constraints([Constraint::Min(0), Constraint::Length(3)]).split(f.area());
            
            let selected_acc = switch_acc_state.selected().and_then(|i| accounts.get(i));
            let title = format!("Accounts: {}", selected_acc.map(|a| a.full_address.as_str()).unwrap_or("None"));
            
            let items: Vec<ListItem> = accounts.iter().map(|acc| {
                ListItem::new(format!("{} {}", acc.full_address, if acc.is_active { "[ON]" } else { "" }))
            }).collect();
            let list = List::new(items).block(Block::default().title(title).borders(Borders::ALL))
                .highlight_style(Style::default().bg(Color::Cyan).fg(Color::Black)).highlight_symbol(">");
            f.render_stateful_widget(list, chunks[0], switch_acc_state);
            f.render_widget(Paragraph::new("[↑/↓] [↵] [Esc] [d]").block(Block::default().borders(Borders::ALL)), chunks[1]);
        }
        AppMode::DeleteAccount => {
            let chunks = Layout::default().direction(Direction::Vertical).margin(1).constraints([Constraint::Min(0)]).split(f.area());
            let items: Vec<ListItem> = accounts.iter().map(|acc| ListItem::new(acc.full_address.as_str())).collect();
            let list = List::new(items).block(Block::default().title("DEL ACC").title_bottom(Line::from("[↵] [Esc]")).borders(Borders::ALL).border_style(Style::default().fg(Color::Red)))
                .highlight_style(Style::default().bg(Color::Red).add_modifier(Modifier::BOLD)).highlight_symbol(">");
            f.render_stateful_widget(list, chunks[0], delete_account_state);
        }
        AppMode::AddContact => {
            let chunks = Layout::default().direction(Direction::Vertical).margin(1).constraints([Constraint::Length(3), Constraint::Min(0)]).split(f.area());
            f.render_widget(Paragraph::new(input_buffer).block(Block::default().title("Add peer | user#peer@cult.net | [↵] [Esc]").borders(Borders::ALL)), chunks[0]);
        }
        AppMode::DeleteContact => {
            let chunks = Layout::default().direction(Direction::Vertical).margin(1).constraints([Constraint::Min(0), Constraint::Length(3)]).split(f.area());
            let items: Vec<ListItem> = contacts.iter().enumerate().map(|(i, c)| {
                ListItem::new(format!("{}{}", c.full_address, if Some(i) == delete_contact_state.selected() { if delete_confirm { " <?confirm" } else { " <" } } else { "" }))
            }).collect();
            let list = List::new(items).block(Block::default().title("DEL PEER").borders(Borders::ALL).border_style(Style::default().fg(Color::Red)))
                .highlight_style(Style::default().bg(Color::Red).add_modifier(Modifier::BOLD)).highlight_symbol(">");
            f.render_stateful_widget(list, chunks[0], delete_contact_state);
            f.render_widget(Paragraph::new("[↵] [Esc]").block(Block::default().borders(Borders::ALL)), chunks[1]);
        }
        AppMode::ShowKey => {
            let chunks = Layout::default().direction(Direction::Vertical).margin(1).constraints([Constraint::Min(0), Constraint::Length(3)]).split(f.area());
            if let Some(account) = account_opt {
                f.render_widget(Paragraph::new(format!("SAVE THIS!\n\n{}", account.private_key)).block(Block::default().title("RSA PRIV").borders(Borders::ALL).style(Style::default().fg(Color::Red))).wrap(ratatui::widgets::Wrap { trim: true }), chunks[0]);
                f.render_widget(Paragraph::new("[↵] [Esc]").block(Block::default().borders(Borders::ALL)), chunks[1]);
            }
        }
        AppMode::Main | AppMode::Peers | AppMode::Chat => {
            let main_chunks = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Length(28), Constraint::Min(10)])
                .split(f.area());

            let sidebar_chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Length(3), Constraint::Min(0)])
                .split(main_chunks[0]);

            let title = if is_offline_mode { " OFFLINE | CULT.NET " } else { " ONLINE | CULT.NET " };
            let acc_str = active_acc_address.clone().unwrap_or_default();
            
            let hints = if is_offline_mode {
                "[Esc] [q]"
            } else {
                match mode {
                    AppMode::Peers => "[↵] [a][d][s][Esc][↑/↓]",
                    AppMode::Chat => " [Esc] ",
                    _ => "[p][s][q]",
                }
            };

            let acc_block = Paragraph::new(Span::styled(acc_str, Style::default().add_modifier(Modifier::BOLD)))
                .block(Block::default().title(title).borders(Borders::ALL));
            f.render_widget(acc_block, sidebar_chunks[0]);

            let mut sidebar_items: Vec<ListItem> = vec![];
            for (i, c) in contacts.iter().enumerate() {
                let mut spans = vec![];
                let is_on = match peer_statuses.get(&c.full_address) {
                    Some(&(true, _)) => true,
                    _ => false,
                };
                let name = c.full_address.split('@').next().unwrap_or(&c.full_address);
                let mut display_name = name.to_string();
                let is_selected = peers_state.selected() == Some(i);

                if !is_selected && display_name.chars().count() > 20 {
                    let chars: String = display_name.chars().take(18).collect();
                    display_name = format!("{}..", chars);
                }

                if is_on {
                    spans.push(Span::styled(format!("● {}", display_name), Style::default().fg(Color::Green)));
                } else {
                    spans.push(Span::styled(format!("○ {}", display_name), Style::default().fg(Color::DarkGray)));
                }

                let unread = unread_counts.get(&c.full_address).copied().unwrap_or(0);
                if unread > 0 {
                    spans.push(Span::styled(format!(" 🔺{}", crate::client::cli::to_superscript(unread)), Style::default().fg(Color::Red)));
                }

                sidebar_items.push(ListItem::new(Line::from(spans)));
            }

            let sidebar_list = List::new(sidebar_items)
                .block(Block::default().borders(Borders::ALL).title_bottom(hints))
                .highlight_style(Style::default().bg(Color::Cyan).fg(Color::Black))
                .highlight_symbol(">");

            f.render_stateful_widget(sidebar_list, sidebar_chunks[1], peers_state);

            let right_chunks = if is_offline_mode {
                Layout::default().direction(Direction::Vertical)
                    .constraints([Constraint::Length(3), Constraint::Min(1)]).split(main_chunks[1])
            } else {
                Layout::default().direction(Direction::Vertical)
                    .constraints([Constraint::Length(3), Constraint::Min(1), Constraint::Length(3)]).split(main_chunks[1])
            };

            if active_peer.is_empty() {
                f.render_widget(Paragraph::new(" Select peer [p] ").block(Block::default().borders(Borders::ALL)), main_chunks[1]);
            } else {
                let peer_short = active_peer.split('@').next().unwrap_or(active_peer);
                let peer_is_on = match peer_status {
                    Some((true, _)) => true,
                    _ => false,
                };
                
                let status_span = if peer_is_on {
                    Span::styled("ONLINE", Style::default().fg(Color::Green))
                } else {
                    Span::styled("OFFLINE", Style::default().fg(Color::Blue))
                };
                
                let header = Paragraph::new(Line::from(vec![
                    Span::raw(format!("{} | ", peer_short)),
                    status_span,
                ])).block(Block::default().borders(Borders::ALL));
                f.render_widget(header, right_chunks[0]);

                if let Some(account) = account_opt {
                    let mut lines = Vec::new();
                    for (sender, text) in message_history {
                        let display_sender = if sender == &account.full_address { "you" } else { sender.split('@').next().unwrap_or(sender) };
                        lines.push(Line::from(format!("{}: {}", display_sender, text)));
                    }

                    let chat_width = right_chunks[1].width.saturating_sub(2) as usize;
                    let mut total_lines: u16 = 0;
                    for (sender, text) in message_history {
                        let display_sender = if sender == &account.full_address { "you" } else { sender.split('@').next().unwrap_or(sender) };
                        let full_line_len = display_sender.len() + 2 + text.chars().count();
                        let visual_rows = if chat_width > 0 { (full_line_len + chat_width - 1) / chat_width } else { 1 };
                        total_lines += visual_rows as u16;
                    }

                    let visible_height = right_chunks[1].height.saturating_sub(2);
                    let mut max_scroll = 0;

                    if total_lines > visible_height {
                        max_scroll = total_lines - visible_height;
                        if *is_autoscroll { *scroll_offset = max_scroll; }
                        else if *scroll_offset > max_scroll { *scroll_offset = max_scroll; }
                        if *scroll_offset == max_scroll { *is_autoscroll = true; }
                    } else {
                        *scroll_offset = 0;
                        *is_autoscroll = true;
                    }

                    let mut messages_block = Block::default().borders(Borders::ALL);
                    if *scroll_offset < max_scroll {
                        messages_block = messages_block.title_top(
                            Line::from(" ▲▲▲ ")
                                .style(Style::default().fg(Color::Yellow))
                                .alignment(ratatui::layout::Alignment::Right)
                        );
                    }

                    let messages = Paragraph::new(lines)
                        .block(messages_block)
                        .wrap(ratatui::widgets::Wrap { trim: true })
                        .scroll((*scroll_offset, 0));
                    f.render_widget(messages, right_chunks[1]);
                }

                if !is_offline_mode && mode == &AppMode::Chat {
                    let input_width = right_chunks[2].width.saturating_sub(2) as usize;
                    let input_scroll = (input_buffer.chars().count()).saturating_sub(input_width) as u16;
                    let input_p = Paragraph::new(input_buffer)
                        .block(Block::default().title("[↵] [↑/↓]").borders(Borders::ALL))
                        .scroll((0, input_scroll));
                    f.render_widget(input_p, right_chunks[2]);
                    
                    let cursor_pos = (input_buffer.chars().count() as u16).min(input_width as u16);
                    f.set_cursor_position((right_chunks[2].x + 1 + cursor_pos, right_chunks[2].y + 1));
                }
            }
        }
    }
}
