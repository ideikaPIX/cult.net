use crossterm::{
    event::{self, Event, KeyCode, KeyModifiers},
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    ExecutableCommand,
};
use ratatui::{
    backend::{Backend, CrosstermBackend},
    layout::{Constraint, Direction, Layout},
    widgets::{Block, Borders, List, ListItem, Paragraph},
    text::Line,
    Frame, Terminal,
};
use std::io::{stdout, Write};
use crate::client::{auth, network, storage};
use crate::shared::{ClientMessage, ServerResponse};
use std::time::Duration;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::sync::mpsc;

#[derive(PartialEq, Clone, Copy)]
enum AppMode {
    EnterServerIp,
    Auth,
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
    let mut message_history: Vec<(String, String)> = Vec::new(); // (sender, text)
    let mut delete_idx: usize = 0;
    let mut delete_confirm: bool = false;
    let mut last_mode = mode;
    let mut scroll_offset: u16 = 0;
    let mut is_autoscroll: bool = true;
    let mut peer_status: Option<(bool, Option<String>)> = None;

    let mut net_client: Option<network::NetworkClient> = None;
    let mut account: Option<storage::Account> = None;
    let mut receiver: Option<Arc<Mutex<mpsc::UnboundedReceiver<ServerResponse>>>> = None;
    let mut connection_error = String::new();

    let active_acc = auth::get_active_account().unwrap_or(None);

    loop {
        // Process network messages
        if let Some(ref r) = receiver {
            if let Ok(mut lock) = r.try_lock() {
                while let Ok(resp) = lock.try_recv() {
                    match resp {
                        ServerResponse::IncomingMessage { from, encrypted_content } => {
                            if let Some(acc) = account.as_ref() {
                                // Auto-decrypt
                                let dec_content = crate::client::crypto::decrypt(&encrypted_content, &acc.private_key)
                                    .unwrap_or_else(|_| format!("🔒 [Decryption failed]: {}", encrypted_content));

                                // If we are in chat with this person, update history
                                if active_peer == from {
                                    message_history.push((from.clone(), dec_content.clone()));
                                    is_autoscroll = true;
                                }
                                
                                // Always save to DB
                                if let Ok(conn) = storage::get_chat_db(&from) {
                                    let ts = chrono::Utc::now().to_rfc3339();
                                    let _ = conn.execute(
                                        "INSERT INTO messages (timestamp, sender, content, status, is_yours) VALUES (?1, ?2, ?3, ?4, ?5)",
                                        (ts, from, dec_content, "received", false),
                                    );
                                }
                            }
                        },
                        ServerResponse::KeyResponse { public_key, online_status: _ } => {
                            // Update contact's public key
                            if let Some(contact) = contacts.iter_mut().find(|c| c.full_address == active_peer) {
                                contact.public_key = public_key.clone();
                                let mut data = storage::load_contacts().unwrap_or_default();
                                if let Some(c) = data.contacts.iter_mut().find(|c| c.full_address == active_peer) {
                                    c.public_key = public_key.clone();
                                    let _ = storage::save_contacts(&data);
                                }
                            }
                        },
                        ServerResponse::StatusResponse { target, online, last_seen } => {
                            if target == active_peer {
                                peer_status = Some((online, last_seen));
                            }
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
            delete_idx,
            delete_confirm,
            &connection_error,
            active_acc.as_ref().map(|a| a.full_address.clone()),
            &mut scroll_offset,
            &mut is_autoscroll,
            &peer_status
        ))?;

        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                // Global quit
                if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
                    return Ok(());
                }

                match mode {
                    AppMode::EnterServerIp => match key.code {
                        KeyCode::Char(c) => input_buffer.push(c),
                        KeyCode::Backspace => { input_buffer.pop(); },
                        KeyCode::Enter => {
                            let ip = input_buffer.trim().to_string();
                            if !ip.is_empty() {
                                connection_error.clear();
                                match tokio::time::timeout(Duration::from_secs(3), network::connect(&ip)).await {
                                    Ok(Ok(client)) => {
                                        receiver = Some(client.receiver.clone());
                                        net_client = Some(client);
                                        input_buffer.clear();
                                        mode = AppMode::Auth;
                                    }
                                    _ => {
                                        connection_error = "❌ Connection failed. Retrying...".to_string();
                                    }
                                }
                            }
                        }
                        KeyCode::Esc => return Ok(()),
                        _ => {}
                    },
                    AppMode::Auth => match key.code {
                        KeyCode::Char(c) => input_buffer.push(c),
                        KeyCode::Backspace => { input_buffer.pop(); },
                        KeyCode::Enter => {
                            let username = input_buffer.trim().to_string();
                            let mut is_new = false;
                            let acc = if username.is_empty() && active_acc.is_some() {
                                active_acc.clone().unwrap()
                            } else {
                                let actual_username = if username.is_empty() { "anonymous" } else { &username };
                                is_new = true;
                                auth::login(actual_username, true).unwrap()
                            };
                            
                            if let Some(c) = net_client.as_mut() {
                                let msg = ClientMessage::Register {
                                    username: acc.username.clone(),
                                    public_key: acc.public_key.clone(),
                                };
                                let _ = c.sender.send(msg);
                            }
                            
                            account = Some(acc);
                            input_buffer.clear();
                            mode = if is_new { AppMode::ShowKey } else { AppMode::Main };
                        }
                        KeyCode::Esc => return Ok(()),
                        _ => {}
                    },
                    AppMode::Main => match key.code {
                        KeyCode::Char('q') => return Ok(()),
                        KeyCode::Char('p') => {
                            contacts = storage::load_contacts().unwrap_or_default().contacts;
                            mode = AppMode::Peers;
                        }
                        KeyCode::Char('s') => {
                            accounts = storage::load_accounts().unwrap_or_default().accounts;
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
                                delete_idx = 0;
                                delete_confirm = false;
                                mode = AppMode::DeleteContact;
                            }
                        }
                        KeyCode::Char(c) if c.is_ascii_digit() => {
                            let idx = c.to_digit(10).unwrap() as usize;
                            if let Some(contact) = contacts.get(idx) {
                                active_peer = contact.full_address.clone();
                                
                                // Fetch public key if missing
                                if contact.public_key.is_empty() {
                                    if let Some(client) = net_client.as_mut() {
                                        let _ = client.sender.send(ClientMessage::GetPublicKey { target: active_peer.clone() });
                                    }
                                }
                                
                                // Always fetch status
                                if let Some(client) = net_client.as_mut() {
                                    let _ = client.sender.send(ClientMessage::CheckStatus { target: active_peer.clone() });
                                }
                                peer_status = None;
                                
                                // Load history
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
                        _ => {}
                    },
                    AppMode::Chat => match key.code {
                        KeyCode::Esc => mode = AppMode::Peers,
                        KeyCode::Up | KeyCode::PageUp => {
                            if scroll_offset > 0 {
                                scroll_offset = scroll_offset.saturating_sub(1);
                            }
                            is_autoscroll = false;
                        }
                        KeyCode::Down | KeyCode::PageDown => {
                            scroll_offset = scroll_offset.saturating_add(1);
                            // It will be clamped to max scroll in `ui` rendering, 
                            // we'll update is_autoscroll inside `ui` if we reach bottom,
                            // or we can just leave it to `ui` to handle clamp and autoscroll flags.
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
                                
                                let (enc_content, db_content) = if peer_pub_key.is_empty() {
                                    (text.clone(), format!("⚠️ UNENCRYPTED: {}", text))
                                } else {
                                    if let Ok(encrypted) = crate::client::crypto::encrypt(&text, &peer_pub_key) {
                                        (encrypted, text.clone())
                                    } else {
                                        (text.clone(), format!("❌ ENCRYPTION FAILED: {}", text))
                                    }
                                };

                                if let Some(client) = net_client.as_mut() {
                                    if let Some(acc) = account.as_ref() {
                                        let msg = ClientMessage::SendMessage {
                                            from: acc.full_address.clone(),
                                            to: active_peer.clone(),
                                            encrypted_content: enc_content,
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
                                        public_key: "".to_string(), // mock
                                        added_at: chrono::Utc::now().to_rfc3339(),
                                        last_message: None,
                                    };
                                    contacts_data.contacts.push(contact);
                                    let _ = storage::save_contacts(&contacts_data);
                                    // Refresh contacts immediately
                                    contacts = contacts_data.contacts;
                                    input_buffer.clear();
                                    mode = AppMode::Peers;
                                }
                            }
                        }
                        _ => {}
                    },
                    AppMode::DeleteContact => match key.code {
                        KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('b') => {
                            mode = AppMode::Peers;
                        }
                        KeyCode::Up => {
                            if delete_idx > 0 {
                                delete_idx -= 1;
                                delete_confirm = false;
                            }
                        }
                        KeyCode::Down => {
                            if delete_idx + 1 < contacts.len() {
                                delete_idx += 1;
                                delete_confirm = false;
                            }
                        }
                        KeyCode::Enter => {
                            if delete_confirm {
                                let mut contacts_data = storage::load_contacts().unwrap_or_default();
                                if delete_idx < contacts_data.contacts.len() {
                                    contacts_data.contacts.remove(delete_idx);
                                    let _ = storage::save_contacts(&contacts_data);
                                    contacts = contacts_data.contacts;
                                }
                                if contacts.is_empty() {
                                    mode = AppMode::Peers;
                                } else {
                                    if delete_idx >= contacts.len() {
                                        delete_idx = contacts.len() - 1;
                                    }
                                    delete_confirm = false;
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
                        KeyCode::Char(c) if c.is_ascii_digit() => {
                            let idx = c.to_digit(10).unwrap() as usize;
                            if let Some(selected_acc) = accounts.get(idx) {
                                if auth::switch_account(&selected_acc.full_address).unwrap_or(false) {
                                    account = Some(selected_acc.clone());
                                }
                            }
                            mode = AppMode::Main;
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
    delete_idx: usize,
    delete_confirm: bool,
    connection_error: &str,
    active_acc_address: Option<String>,
    scroll_offset: &mut u16,
    is_autoscroll: &mut bool,
    peer_status: &Option<(bool, Option<String>)>,
) {
    f.render_widget(ratatui::widgets::Clear, f.area());
    match mode {
        AppMode::EnterServerIp => {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .margin(2)
                .constraints([Constraint::Length(3), Constraint::Min(0)])
                .split(f.area());

            let text = if connection_error.is_empty() {
                input_buffer.to_string()
            } else {
                format!("{}\n{}", input_buffer, connection_error)
            };
            let input = Paragraph::new(text).block(Block::default().title("🌐 Connect to CULT.NET Server\nEnter Server IP or Domain (e.g., 195.133.14.56 or localhost:8080) and press Enter").borders(Borders::ALL));
            f.render_widget(input, chunks[0]);
        }
        AppMode::Auth => {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .margin(2)
                .constraints([Constraint::Length(3), Constraint::Min(0)])
                .split(f.area());

            let title = if let Some(addr) = active_acc_address {
                format!("Enter username (press Enter to use {})", addr)
            } else {
                "Enter username".to_string()
            };
            let input = Paragraph::new(input_buffer).block(Block::default().title(title).borders(Borders::ALL));
            f.render_widget(input, chunks[0]);
        }
        AppMode::Main => {
            if let Some(account) = account_opt {
                let chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .margin(2)
                    .constraints([Constraint::Length(6), Constraint::Min(0)])
                    .split(f.area());

                let status_text = if is_online {
                    format!("✓ connected | {} | 0 peers | 0 unread", account.full_address)
                } else {
                    format!("✗ offline mode (only read local saved messages)\n⚠️ {} (unconfirmed)", account.full_address)
                };

                let block = Block::default().title("🔐 cult.net").borders(Borders::ALL);
                let paragraph = Paragraph::new(status_text).block(block);
                f.render_widget(paragraph, chunks[0]);

                let menu = Paragraph::new("[p]eers  [s]witch  [q]uit");
                f.render_widget(menu, chunks[1]);
            }
        }
        AppMode::Peers => {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .margin(2)
                .constraints([Constraint::Min(0), Constraint::Length(3)])
                .split(f.area());

            let items: Vec<ListItem> = contacts
                .iter()
                .enumerate()
                .map(|(i, c)| {
                    ListItem::new(format!("[{}] {}       ✓", i, c.full_address))
                })
                .collect();

            let list = List::new(items).block(Block::default().title("Peers").borders(Borders::ALL));
            f.render_widget(list, chunks[0]);

            let instructions = Paragraph::new("Type [number] to chat | [b]ack / Esc | [d]elete / [a]dd").block(Block::default().borders(Borders::ALL));
            f.render_widget(instructions, chunks[1]);
        }
        AppMode::Chat => {
            if let Some(account) = account_opt {
                let chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .margin(1)
                    .constraints([Constraint::Min(0), Constraint::Length(3)])
                    .split(f.area());

                let mut lines = Vec::new();
                for (sender, text) in message_history {
                    let display_sender = if sender == &account.full_address { "you" } else { sender.as_str() };
                    lines.push(Line::from(format!("{}: {}", display_sender, text)));
                }

                let total_lines = lines.len() as u16;
                let visible_height = chunks[0].height.saturating_sub(2);

                if total_lines > visible_height {
                    let max_scroll = total_lines - visible_height;
                    if *is_autoscroll {
                        *scroll_offset = max_scroll;
                    } else if *scroll_offset > max_scroll {
                        *scroll_offset = max_scroll;
                    }
                    if *scroll_offset == max_scroll {
                        *is_autoscroll = true;
                    }
                } else {
                    *scroll_offset = 0;
                    *is_autoscroll = true;
                }

                let scroll_indicator = if total_lines > visible_height && !*is_autoscroll {
                    " [Auto-scroll OFF]"
                } else {
                    ""
                };

                use ratatui::text::Span;
                use ratatui::style::{Style, Color};

                let status_span = match peer_status {
                    Some((true, _)) => Span::styled(" [Online]", Style::default().fg(Color::Green)),
                    Some((false, Some(last_seen))) => {
                        if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(last_seen) {
                            let now = chrono::Utc::now();
                            let diff = now.signed_duration_since(dt.with_timezone(&chrono::Utc));
                            let text = if diff.num_minutes() < 60 {
                                format!(" [Last seen: {}m ago]", diff.num_minutes())
                            } else if diff.num_hours() < 24 {
                                format!(" [Last seen: {}h ago]", diff.num_hours())
                            } else {
                                format!(" [Last seen: {}d ago]", diff.num_days())
                            };
                            Span::styled(text, Style::default().fg(Color::DarkGray))
                        } else {
                            Span::styled(" [Offline]", Style::default().fg(Color::DarkGray))
                        }
                    },
                    Some((false, None)) => Span::styled(" [Offline]", Style::default().fg(Color::DarkGray)),
                    None => Span::raw(""),
                };

                let title_line = Line::from(vec![
                    Span::raw(format!("Chat with: {}", active_peer)),
                    status_span,
                    Span::raw(scroll_indicator),
                ]);

                let messages = Paragraph::new(lines)
                    .block(Block::default().title(title_line).borders(Borders::ALL))
                    .scroll((*scroll_offset, 0));
                f.render_widget(messages, chunks[0]);

                let input = Paragraph::new(input_buffer).block(Block::default().title("Type message (Enter to send, Esc to back)").borders(Borders::ALL));
                f.render_widget(input, chunks[1]);
            }
        }
        AppMode::AddContact => {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .margin(2)
                .constraints([Constraint::Length(3), Constraint::Length(2), Constraint::Min(0)])
                .split(f.area());

            let input = Paragraph::new(input_buffer).block(Block::default().title("Add Contact").borders(Borders::ALL));
            f.render_widget(input, chunks[0]);
            
            let is_valid = input_buffer.is_empty() || (input_buffer.ends_with("@cult.net") 
                && input_buffer.chars().filter(|&c| c == '#').count() == 1 
                && input_buffer.chars().filter(|&c| c == '@').count() == 1);
                
            let hint_text = if is_valid {
                "Format: user#peer@cult.net\n(Enter to submit, Esc to cancel)"
            } else {
                "⚠️ Invalid format! Must be: user#peer@cult.net\n(Enter to submit, Esc to cancel)"
            };

            let hint = Paragraph::new(hint_text);
            f.render_widget(hint, chunks[1]);
        }
        AppMode::DeleteContact => {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .margin(2)
                .constraints([Constraint::Min(0), Constraint::Length(3)])
                .split(f.area());

            let items: Vec<ListItem> = contacts
                .iter()
                .enumerate()
                .map(|(i, c)| {
                    let mut text = format!("[{}] {}       ✓", i, c.full_address);
                    if i == delete_idx {
                        if delete_confirm {
                            text.push_str(" <?enter to confirm");
                        } else {
                            text.push_str(" <");
                        }
                    }
                    ListItem::new(text)
                })
                .collect();

            let list = List::new(items).block(Block::default().title("Delete Contact").borders(Borders::ALL));
            f.render_widget(list, chunks[0]);

            let instructions = Paragraph::new("Use [Up/Down] arrows to select | [Enter] to delete | [b]ack / Esc").block(Block::default().borders(Borders::ALL));
            f.render_widget(instructions, chunks[1]);
        }
        AppMode::Switch => {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .margin(2)
                .constraints([Constraint::Min(0), Constraint::Length(3)])
                .split(f.area());

            let items: Vec<ListItem> = accounts
                .iter()
                .enumerate()
                .map(|(i, acc)| {
                    let status = if acc.is_active { "✓ active" } else { "" };
                    ListItem::new(format!("[{}] {} {}", i, acc.full_address, status))
                })
                .collect();

            let list = List::new(items).block(Block::default().title("--- Switch Account ---").borders(Borders::ALL));
            f.render_widget(list, chunks[0]);

            let instructions = Paragraph::new("Type [number] to switch | Esc to back").block(Block::default().borders(Borders::ALL));
            f.render_widget(instructions, chunks[1]);
        }
        AppMode::ShowKey => {
            if let Some(account) = account_opt {
                let chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .margin(2)
                    .constraints([Constraint::Min(0), Constraint::Length(3)])
                    .split(f.area());

                let warning = "COPY YOUR KEY! If you proceed, access to it will be permanently lost.\n\n";
                let text = format!("{}{}", warning, account.private_key);
                
                let block = Block::default()
                    .title("⚠️ IMPORTANT: Your Private Key ⚠️")
                    .borders(Borders::ALL)
                    .style(ratatui::style::Style::default().fg(ratatui::style::Color::Red));
                    
                let paragraph = Paragraph::new(text).block(block).wrap(ratatui::widgets::Wrap { trim: true });
                f.render_widget(paragraph, chunks[0]);

                let instructions = Paragraph::new("Press [Enter] or [Esc] to continue...")
                    .block(Block::default().borders(Borders::ALL));
                f.render_widget(instructions, chunks[1]);
            }
        }
    }
}
