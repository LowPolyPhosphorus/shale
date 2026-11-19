use std::{env, fs, io, path::{Path, PathBuf}};

use ratatui::{
    crossterm::event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind, MouseEventKind, MouseButton},
    crossterm::execute,
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    widgets::{Block, List, ListItem, ListState, Paragraph, Wrap},
    DefaultTerminal, Frame,
};
use walkdir::WalkDir;
use regex::Regex;

#[derive(PartialEq)]
enum Focus {
    Notes,
    Links,
}

struct Note {
    name: String,
    path: PathBuf,
    content: String,
    links: Vec<String>,
}

struct App {
    notes: Vec<Note>,
    list_state: ListState,
    link_state: ListState,
    focus: Focus,
    should_quit: bool,
    preview_scroll: u16,
    history: Vec<usize>,
    forward: Vec<usize>,
    notes_area: Rect,
    links_rect: Rect,
    back_rect: Rect,
}

impl App {
    fn new(vault: &Path) -> Self {
        let link_re = Regex::new(r"\[\[([^\]|#]+)(?:[#|][^\]]*)?\]\]").unwrap();

        let mut notes: Vec<Note> = WalkDir::new(vault)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().map_or(false, |ext| ext == "md"))
            .map(|e| {
                let path = e.path().to_path_buf();
                let content = fs::read_to_string(&path).unwrap_or_default();
                let links: Vec<String> = link_re
                    .captures_iter(&content)
                    .map(|c| c[1].trim().to_lowercase())
                    .collect();
                Note {
                    name: path
                        .file_stem()
                        .map(|s| s.to_string_lossy().to_string())
                        .unwrap_or_default(),
                    path,
                    content,
                    links,
                }
            })
            .collect();
        
        notes.sort_by_key(|n| n.name.to_lowercase());

        let mut list_state = ListState::default();
        if !notes.is_empty() {
            list_state.select(Some(0));
        }

        let mut app = App {
            notes,
            list_state,
            link_state: ListState::default(),
            focus: Focus::Notes,
            should_quit: false,
            preview_scroll: 0,
            history: Vec::new(),
            forward: Vec::new(),
            notes_area: Rect::default(),
            links_rect: Rect::default(),
            back_rect: Rect::default(),
        };
        app.reset_link_selection();
        app
    }
    
    fn selected_note(&self) -> Option<&Note> {
        self.list_state.selected().and_then(|i| self.notes.get(i))
    }

    fn note_exists(&self, lowercase_name: &str) -> bool {
        self.notes
            .iter()
            .any(|n| n.name.to_lowercase() == lowercase_name)
    }

    fn reset_link_selection(&mut self) {
        let has_links = self.selected_note().map_or(false, |n| !n.links.is_empty());
        self.link_state.select(if has_links { Some(0) } else { None });
        self.preview_scroll = 0;
    }

    fn scroll_preview(&mut self, delta: i32) {
        self.preview_scroll = (self.preview_scroll as i32 + delta).max(0) as u16;
    }

    fn move_note_selection(&mut self, delta: i32) {
        if self.notes.is_empty() {
            return;
        }
        let cur = self.list_state.selected().unwrap_or(0) as i32;
        let next = (cur + delta).clamp(0, self.notes.len() as i32 - 1) as usize;
        self.list_state.select(Some(next));
        self.reset_link_selection();
    }

    fn follow_link(&mut self) {
        let Some(note) = self.selected_note() else { return };
        let Some(i) = self.link_state.selected() else { return };
        let Some(target) = note.links.get(i) else { return };
        let found = self
            .notes
            .iter()
            .position(|n| n.name.to_lowercase() == *target);
        if let Some(idx) = found {
            if let Some(cur) = self.list_state.selected() {
                self.history.push(cur);
            }
            self.forward.clear();
            self.list_state.select(Some(idx));
            self.reset_link_selection();
        }
    }

    fn go_back(&mut self) {
        if let Some(prev) = self.history.pop() {
            if let Some(cur) = self.list_state.selected() {
                self.forward.push(cur);
            }
            self.list_state.select(Some(prev));
            self.reset_link_selection();
        }
    }

    fn go_forward(&mut self) {
        if let Some(next) = self.forward.pop() {
            if let Some(cur) = self.list_state.selected() {
                self.history.push(cur);
            }
            self.list_state.select(Some(next));
            self.reset_link_selection();
        }
    }

    fn handle_click(&mut self, x: u16, y: u16) {
        if let Some(row) = row_in(self.notes_area, x, y) {
            let idx = row + self.list_state.offset();
            if idx < self.notes.len() {
                self.focus = Focus::Notes;
                self.list_state.select(Some(idx));
                self.reset_link_selection();
            }
        } else if let Some(row) = row_in(self.links_rect, x, y) {
            let idx = row + self.link_state.offset();
            let count = self.selected_note().map_or(0, |n| n.links.len());
            if idx < count {
                self.focus = Focus::Links;
                self.link_state.select(Some(idx));
                self.follow_link();
            }
        } else if let Some(row) = row_in(self.back_rect, x, y) {
            let target = self.backlinks().get(row).map(|n| n.path.clone());
            if let Some(path) = target {
                if let Some(idx) = self.notes.iter().position(|n| n.path == path) {
                    if let Some(cur) = self.list_state.selected() {
                        self.history.push(cur);
                    }
                    self.forward.clear();
                    self.list_state.select(Some(idx));
                    self.reset_link_selection();
                }
            }
        }
    }

    fn backlinks(&self) -> Vec<&Note> {
        let Some(sel) = self.selected_note() else {
            return Vec::new();
        };
        let target = sel.name.to_lowercase();
        self.notes
            .iter()
            .filter(|n| n.path != sel.path && n.links.contains(&target))
            .collect()
    } 

    fn handle_key(&mut self, code: KeyCode) {
        match code {
            KeyCode::Char('q') | KeyCode::Esc => self.should_quit = true,
            KeyCode::Tab => {
                self.focus = match self.focus {
                    Focus::Notes => Focus::Links,
                    Focus::Links => Focus::Notes,
                };
            }
            KeyCode::Down | KeyCode::Char('j') => match self.focus {
                Focus::Notes => self.move_note_selection(1),
                Focus::Links => self.link_state.select_next(),
            },
            KeyCode::Up | KeyCode::Char('k') => match self.focus {
                Focus::Notes => self.move_note_selection(-1),
                Focus::Links => self.link_state.select_previous(),
            },
            KeyCode::Enter => {
                if self.focus == Focus::Links {
                    self.follow_link();
                }
            }
            KeyCode::PageDown => self.scroll_preview(10),
            KeyCode::PageUp => self.scroll_preview(-10),
            KeyCode::Backspace | KeyCode::Char('h') => self.go_back(),
            KeyCode::Char('l') => self.go_forward(),
            _ => {}
        }
    }
}

fn row_in(area: Rect, x: u16, y: u16) -> Option<usize> {
    if x > area.x
        && x < area.x + area.width.saturating_sub(1)
        && y > area.y 
        && y < area.y + area.height.saturating_sub(1)
    {
        Some((y - area.y - 1) as usize)
    } else {
        None
    }
}

fn ui(frame: &mut Frame, app: &mut App) {
    let [left, right] =
        Layout::horizontal([Constraint::Percentage(30), Constraint::Percentage(70)])
            .areas(frame.area());
    let [preview_area, links_area] =
        Layout::vertical([Constraint::Min(5), Constraint::Length(10)]).areas(right);
    let [out_area, back_area] = 
        Layout::horizontal([Constraint::Percentage(50), Constraint::Percentage(50)])
            .areas(links_area);
    app.notes_area = left;
    app.links_rect = out_area;
    app.back_rect = back_area;
    
    // Left pane: note list
    let items: Vec<ListItem> = app
        .notes
        .iter()
        .map(|n| ListItem::new(n.name.as_str()))
        .collect();
    let list = List::new(items)
        .block(
            Block::bordered()
                .title(" Notes ")
                .border_style(if app.focus == Focus::Notes {
                    Style::new().fg(Color::Yellow)
                } else {
                    Style::new()
                }),
        )
        .highlight_style(Style::new().add_modifier(Modifier::REVERSED))
        .highlight_symbol("> ");
    frame.render_stateful_widget(list, left, &mut app.list_state);

    let (title, content) = match app.selected_note() {
        Some(n) => (format!(" {} ", n.name), n.content.clone()),
        None => (" Preview ".to_string(), "No notes found.".to_string()),
    };
    let max_scroll = content.lines().count().saturating_sub(1) as u16;
    app.preview_scroll = app.preview_scroll.min(max_scroll) as u16;
    let preview = Paragraph::new(content)
        .block(Block::bordered().title(title))
        .wrap(Wrap { trim: false })
        .scroll((app.preview_scroll, 0));
    frame.render_widget(preview, preview_area);

    let outgoing: Vec<ListItem> = app
        .selected_note()
        .map(|n| {
            n.links
                .iter()
                .map(|l| {
                    if app.note_exists(l) {
                        ListItem::new(l.clone())
                    } else {
                        ListItem::new(format!("{l} (missing)"))
                    }
                })
                .collect()
        })
        .unwrap_or_default();
    let out_title = format!(" Links ({}) ", outgoing.len());
    let out_list = List::new(outgoing)
        .block(
            Block::bordered()
                .title(out_title)
                .border_style(if app.focus == Focus::Links {
                    Style::new().fg(Color::Yellow)
                } else {
                    Style::new()
                }),
        )
        .highlight_style(Style::new().add_modifier(Modifier::REVERSED))
        .highlight_symbol("> ");
    frame.render_stateful_widget(out_list, out_area, &mut app.link_state);

    let backs: Vec<ListItem> = app
        .backlinks()
        .iter()
        .map(|n| ListItem::new(n.name.clone()))
        .collect();
    let back_title = format!(" Backlinks ({}) ", backs.len());
    frame.render_widget(
        List::new(backs).block(Block::bordered().title(back_title)),
        back_area,
    );
}

fn run(mut terminal: DefaultTerminal, app: &mut App) -> io::Result<()> {
    while !app.should_quit {
        terminal.draw(|f| ui(f, app))?;
        match event::read()? {
            Event::Key(key) if key.kind == KeyEventKind::Press => app.handle_key(key.code),
            Event::Mouse(mouse) => match mouse.kind {
                MouseEventKind::ScrollDown => app.scroll_preview(3),
                MouseEventKind::ScrollUp => app.scroll_preview(-3),
                MouseEventKind::Down(MouseButton::Left) => {
                    app.handle_click(mouse.column, mouse.row)
                }
                _ => {}
            },
            _ => {}
        }
    }
    Ok(())
}

fn main() -> io::Result<()> {
    let vault = env::args().nth(1).unwrap_or_else(|| ".".to_string());
    let mut app = App::new(Path::new(&vault));

    let terminal = ratatui::init();
    execute!(io::stdout(), EnableMouseCapture)?;
    let result = run(terminal, &mut app);
    execute!(io::stdout(), DisableMouseCapture)?;
    ratatui::restore();
    result
}