import re

with open('/home/fistrada/HRMS/cult.net/src/client/cli.rs', 'r') as f:
    content = f.read()

# Find the start of fn ui
match = re.search(r'^fn ui\(\n.*?^}  # End of UI?', content, re.MULTILINE | re.DOTALL)
# Actually it's easier to find the index of "fn ui(" and the end of the file, since it's at the end.
start_idx = content.find("fn ui(\n")
if start_idx == -1:
    print("Could not find fn ui")
    exit(1)

new_ui = """fn ui(
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
                format!("{} Connecting", spinner_frames[current_frame])
            } else if !connection_error.is_empty() {
                "Conn fail. [Esc] Exit | [Enter] Offline mode".to_string()
            } else {
                input_buffer.to_string()
            };
            let title = if connection_error.is_empty() { "Server IP" } else { "Error" };
            let input = Paragraph::new(text).block(Block::default().title(title).borders(Borders::ALL));
            f.render_widget(input, chunks[0]);

            if !server_history.is_empty() {
                let items: Vec<ListItem> = server_history.iter().map(|ip| ListItem::new(ip.as_str())).collect();
                let history_list = List::new(items)
                    .block(Block::default().title("Recent").borders(Borders::ALL))
                    .highlight_style(Style::default().bg(Color::DarkGray).add_modifier(Modifier::BOLD))
                    .highlight_symbol(">");
                f.render_stateful_widget(history_list, chunks[1], server_history_state);
            }
        }
        AppMode::Auth => {
            let chunks = Layout::default().direction(Direction::Vertical).margin(1)
                .constraints([Constraint::Length(3), Constraint::Min(0)]).split(f.area());
            let text = if is_generating_keys { format!("{} Gen RSA...", spinner_frames[current_frame]) } else { input_buffer.to_string() };
            let input = Paragraph::new(text).block(Block::default().title("User [Right] sw").borders(Borders::ALL));
            f.render_widget(input, chunks[0]);
        }
        AppMode::AuthSwitch | AppMode::Switch => {
            let chunks = Layout::default().direction(Direction::Vertical).margin(1)
                .constraints([Constraint::Min(0), Constraint::Length(3)]).split(f.area());
            let items: Vec<ListItem> = accounts.iter().map(|acc| {
                ListItem::new(format!("{} {}", acc.full_address, if acc.is_active { "[ON]" } else { "" }))
            }).collect();
            let list = List::new(items).block(Block::default().title("Accounts").borders(Borders::ALL))
                .highlight_style(Style::default().bg(Color::DarkGray).add_modifier(Modifier::BOLD)).highlight_symbol(">");
            f.render_stateful_widget(list, chunks[0], switch_acc_state);
            f.render_widget(Paragraph::new("[Up/Down] Nav | [Enter] Sel | [Esc] Bk | [d]el").block(Block::default().borders(Borders::ALL)), chunks[1]);
        }
        AppMode::DeleteAccount => {
            let chunks = Layout::default().direction(Direction::Vertical).margin(1).constraints([Constraint::Min(0)]).split(f.area());
            let items: Vec<ListItem> = accounts.iter().map(|acc| ListItem::new(acc.full_address.as_str())).collect();
            let list = List::new(items).block(Block::default().title("DEL ACC").title_bottom(Line::from("[Enter] Del | [Esc] Bk")).borders(Borders::ALL).border_style(Style::default().fg(Color::Red)))
                .highlight_style(Style::default().bg(Color::Red).add_modifier(Modifier::BOLD)).highlight_symbol(">");
            f.render_stateful_widget(list, chunks[0], delete_account_state);
        }
        AppMode::AddContact => {
            let chunks = Layout::default().direction(Direction::Vertical).margin(1).constraints([Constraint::Length(3), Constraint::Min(0)]).split(f.area());
            f.render_widget(Paragraph::new(input_buffer).block(Block::default().title("Add: user#peer@cult.net [Enter] Ok [Esc] Bk").borders(Borders::ALL)), chunks[0]);
        }
        AppMode::DeleteContact => {
            let chunks = Layout::default().direction(Direction::Vertical).margin(1).constraints([Constraint::Min(0), Constraint::Length(3)]).split(f.area());
            let items: Vec<ListItem> = contacts.iter().enumerate().map(|(i, c)| {
                ListItem::new(format!("{}{}", c.full_address, if Some(i) == delete_contact_state.selected() { if delete_confirm { " <?ok" } else { " <" } } else { "" }))
            }).collect();
            let list = List::new(items).block(Block::default().title("DEL PEER").borders(Borders::ALL).border_style(Style::default().fg(Color::Red)))
                .highlight_style(Style::default().bg(Color::Red).add_modifier(Modifier::BOLD)).highlight_symbol(">");
            f.render_stateful_widget(list, chunks[0], delete_contact_state);
            f.render_widget(Paragraph::new("[Enter] Del | [Esc] Bk").block(Block::default().borders(Borders::ALL)), chunks[1]);
        }
        AppMode::ShowKey => {
            let chunks = Layout::default().direction(Direction::Vertical).margin(1).constraints([Constraint::Min(0), Constraint::Length(3)]).split(f.area());
            if let Some(account) = account_opt {
                f.render_widget(Paragraph::new(format!("SAVE THIS!\\n\\n{}", account.private_key)).block(Block::default().title("RSA PRIV").borders(Borders::ALL).style(Style::default().fg(Color::Red))).wrap(ratatui::widgets::Wrap { trim: true }), chunks[0]);
                f.render_widget(Paragraph::new("[Enter/Esc] Ok").block(Block::default().borders(Borders::ALL)), chunks[1]);
            }
        }
        AppMode::Main | AppMode::Peers | AppMode::Chat => {
            let main_chunks = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Length(24), Constraint::Min(10)])
                .split(f.area());

            let title = if is_offline_mode { "🌐 CULT [OFF]" } else { "🔐 CULT.NET" };
            let acc_str = format!("Acc: {}", active_acc_address.clone().unwrap_or_default());
            let mut sidebar_items: Vec<ListItem> = vec![
                ListItem::new(Span::styled(acc_str, Style::default().add_modifier(Modifier::BOLD))),
                ListItem::new(""),
            ];

            for c in contacts {
                let mut spans = vec![];
                let is_on = match peer_statuses.get(&c.full_address) {
                    Some(&(true, _)) => true,
                    _ => false,
                };
                let name = c.full_address.split('@').next().unwrap_or(&c.full_address);
                if is_on {
                    spans.push(Span::styled(format!("● {}", name), Style::default().fg(Color::Green)));
                } else {
                    spans.push(Span::styled(format!("○ {}", name), Style::default().fg(Color::DarkGray)));
                }
                spans.push(Span::raw(if is_on { " [ON]" } else { " [OFF]" }));

                let unread = unread_counts.get(&c.full_address).copied().unwrap_or(0);
                if unread > 0 {
                    spans.push(Span::styled(format!(" 🔴{}", crate::client::cli::to_superscript(unread)), Style::default().fg(Color::Red)));
                }

                sidebar_items.push(ListItem::new(Line::from(spans)));
            }

            let sidebar_list = List::new(sidebar_items)
                .block(Block::default().title(title).borders(Borders::ALL).title_bottom("[a]dd [d]el [s]w [q]t"))
                .highlight_style(Style::default().bg(Color::DarkGray))
                .highlight_symbol(">");

            let mut adj_state = ListState::default();
            if mode == &AppMode::Peers {
                if let Some(s) = peers_state.selected() {
                    adj_state.select(Some(s + 2));
                } else if !contacts.is_empty() {
                    adj_state.select(Some(2));
                }
                f.render_stateful_widget(sidebar_list, main_chunks[0], &mut adj_state);
            } else {
                f.render_widget(sidebar_list, main_chunks[0]);
            }

            let right_chunks = if is_offline_mode {
                Layout::default().direction(Direction::Vertical)
                    .constraints([Constraint::Length(3), Constraint::Min(1)]).split(main_chunks[1])
            } else {
                Layout::default().direction(Direction::Vertical)
                    .constraints([Constraint::Length(3), Constraint::Min(1), Constraint::Length(3)]).split(main_chunks[1])
            };

            if active_peer.is_empty() {
                f.render_widget(Paragraph::new(" Select peer from list ").block(Block::default().borders(Borders::ALL)), main_chunks[1]);
            } else {
                let peer_short = active_peer.split('@').next().unwrap_or(active_peer);
                let header = Paragraph::new(format!("Chat: {} | {}", peer_short, if is_offline_mode { "OFFLINE" } else { "ONLINE" }))
                    .block(Block::default().borders(Borders::ALL));
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
                    let max_scroll = if total_lines > visible_height { total_lines - visible_height } else { 0 };

                    if total_lines > visible_height {
                        if *is_autoscroll { *scroll_offset = max_scroll; }
                        else if *scroll_offset > max_scroll { *scroll_offset = max_scroll; }
                        if *scroll_offset == max_scroll { *is_autoscroll = true; }
                    } else {
                        *scroll_offset = 0;
                        *is_autoscroll = true;
                    }

                    let messages = Paragraph::new(lines)
                        .block(Block::default().borders(Borders::ALL))
                        .wrap(ratatui::widgets::Wrap { trim: true })
                        .scroll((*scroll_offset, 0));
                    f.render_widget(messages, right_chunks[1]);
                }

                if !is_offline_mode && mode == &AppMode::Chat {
                    let input_width = right_chunks[2].width.saturating_sub(2) as usize;
                    let input_scroll = (input_buffer.chars().count()).saturating_sub(input_width) as u16;
                    let input_p = Paragraph::new(input_buffer)
                        .block(Block::default().title("[Enter] Send | [Esc] Bk").borders(Borders::ALL))
                        .scroll((0, input_scroll));
                    f.render_widget(input_p, right_chunks[2]);
                    
                    let cursor_pos = (input_buffer.chars().count() as u16).min(input_width as u16);
                    f.set_cursor(ratatui::layout::Position { x: right_chunks[2].x + 1 + cursor_pos, y: right_chunks[2].y + 1 });
                }
            }
        }
    }
}
"""

with open('/home/fistrada/HRMS/cult.net/src/client/cli.rs', 'w') as f:
    f.write(content[:start_idx] + new_ui)

