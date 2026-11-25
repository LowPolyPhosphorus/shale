use std::time::{Duration, Instant};
use std::{env, fs, io, path::{Path, PathBuf}};

use ratatui::crossterm::style::style;
use ratatui::{
    crossterm::{
        event::{
            self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEvent,
            KeyEventKind, KeyModifiers, MouseButton, MouseEventKind,
        },
        execute,
    },
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    symbols::Marker,
    text::{Line, Span},
    widgets::{
        canvas::{Canvas, Line as CanvasLine},
        Block, Clear, List, ListItem, ListState, Paragraph, Wrap,
    },
    DefaultTerminal, Frame,
};
use walkdir::WalkDir;
use regex::Regex;
use tui_textarea::{CursorMove, TextArea};
use std::process::Command;

#[derive(PartialEq)]
enum Focus {
    Notes,
    Links,
    Preview,
}

struct TaskItem {
    note: usize,
    line: usize,
    done: bool,
    text: String,
}

struct Note {
    name: String,
    path: PathBuf,
    content: String,
    links: Vec<String>,
    modified: std::time::SystemTime,
    rel: String,
}

struct App {
    notes: Vec<Note>,
    list_state: ListState,
    link_state: ListState,
    focus: Focus,
    should_quit: bool,
    preview_scroll: u16,
    cursor_row: usize,
    history: Vec<usize>,
    forward: Vec<usize>,
    notes_area: Rect,
    links_rect: Rect,
    back_rect: Rect,
    vault: PathBuf,
    switcher_open: bool,
    query: String,
    matches: Vec<usize>,
    switcher_state: ListState,
    edit_requested: bool,
    prompt_open: bool,
    prompt_text: String,
    editor: Option<TextArea<'static>>,
    editor_dirty: bool,
    discard_warned: bool,
    search_open: bool,
    search_query: String,
    search_results: Vec<(usize, usize, String)>,
    search_state: ListState,
    tag_open: bool,
    tag_list: Vec<(String, Vec<usize>)>,
    tag_stage: Option<usize>,
    tag_state: ListState,
    graph_open: bool,
    graph_center: usize,
    graph_adj: Vec<Vec<usize>>,
    graph_depth: u8,
    graph_sel: usize,
    help_open: bool,
    ac_open: bool,
    ac_query: String,
    ac_matches: Vec<usize>,
    ac_state: ListState,
    status: String,
    confirm_delete: bool,
    prompt_rename: bool,
    tasks_open: bool,
    tasks: Vec<TaskItem>,
    task_state: ListState,
    tasks_show_done: bool,
    recent: Vec<PathBuf>,
    recent_items: Vec<usize>,
    recent_open: bool,
    recent_state: ListState,
    dwell_path: Option<PathBuf>,
    dwell_since: Instant,
    stamp: (usize, u128),
    last_check: Instant,
    health_open: bool,
    health_items: Vec<(usize, String)>,
    health_state: ListState,
    trash_open: bool,
    trash_items: Vec<(PathBuf, String)>,
    trash_state: ListState,
    sort_modified: bool,
    outline_open: bool,
    outline_items: Vec<(usize, usize, String)>,
    outline_state: ListState,
    show_left: bool,
    show_links: bool,
    vault_open: bool,
    vault_items: Vec<PathBuf>,
    vault_state: ListState,
    help_scroll: u16,
    keymap: std::collections::HashMap<char, char>,
}

impl App {
    fn new(vault: &Path) -> Self {
        let link_re = Regex::new(r"\[\[([^\]|]+)(?:[|][^\]]*)?\]\]").unwrap();

        let mut notes: Vec<Note> = WalkDir::new(vault)
            .into_iter()
            .filter_entry(|e| e.depth() == 0 || !e.file_name().to_string_lossy().starts_with('.'))
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
                    modified: fs::metadata(&path)
                        .and_then(|m| m.modified())
                        .unwrap_or(std::time::UNIX_EPOCH),
                    rel: path
                        .strip_prefix(vault)
                        .unwrap_or(path.as_path())
                        .with_extension("")
                        .to_string_lossy()
                        .replace('\\', "/"),
                    path,
                    content,
                    links,
                }
            })
            .collect();

        notes.sort_by_key(|n| n.rel.to_lowercase());

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
            cursor_row: 0,
            history: Vec::new(),
            forward: Vec::new(),
            notes_area: Rect::default(),
            links_rect: Rect::default(),
            back_rect: Rect::default(),
            vault: vault.to_path_buf(),
            switcher_open: false,
            query: String::new(),
            matches: Vec::new(),
            switcher_state: ListState::default(),
            edit_requested: false,
            prompt_open: false,
            prompt_text: String::new(),
            editor: None,
            editor_dirty: false,
            discard_warned: false,
            search_open: false,
            search_query: String::new(),
            search_results: Vec::new(),
            search_state: ListState::default(),
            tag_open: false,
            tag_list: Vec::new(),
            tag_stage: None,
            tag_state: ListState::default(),
            graph_open: false,
            graph_center: 0,
            graph_adj: Vec::new(),
            graph_depth: 2,
            graph_sel: 0,
            help_open: false,
            ac_open: false,
            ac_query: String::new(),
            ac_matches: Vec::new(),
            ac_state: ListState::default(),
            status: String::new(),
            confirm_delete: false,
            prompt_rename: false,
            tasks_open: false,
            tasks: Vec::new(),
            task_state: ListState::default(),
            tasks_show_done: false,
            recent: Vec::new(),
            recent_items: Vec::new(),
            recent_open: false,
            recent_state: ListState::default(),
            trash_open: false,
            trash_items: Vec::new(),
            trash_state: ListState::default(),
            health_open: false,
            health_items: Vec::new(),
            health_state: ListState::default(),
            dwell_path: None,
            dwell_since: Instant::now(),
            stamp: vault_stamp(vault),
            last_check: Instant::now(),
            sort_modified: false,
            outline_open: false,
            outline_items: Vec::new(),
            outline_state: ListState::default(),
            show_left: true,
            show_links: true,
            vault_open: false,
            vault_items: Vec::new(),
            vault_state: ListState::default(),
            help_scroll: 0,
            keymap: load_keymap(),
        };
        app.reset_link_selection();
        app
    }
    
    fn selected_note(&self) -> Option<&Note> {
        self.list_state.selected().and_then(|i| self.notes.get(i))
    }

    fn note_exists(&self, lowercase_name: &str) -> bool {
        resolve_link(&self.notes, lowercase_name).is_some()
    }

    fn reset_link_selection(&mut self) {
        let has_links = self.selected_note().map_or(false, |n| !n.links.is_empty());
        self.link_state.select(if has_links { Some(0) } else { None });
        self.preview_scroll = 0;
        self.cursor_row = 0;
    }

    fn move_view_cursor(&mut self, delta: i32) {
        self.cursor_row = (self.cursor_row as i32 + delta).max(0) as usize;
    }

    fn scroll_preview(&mut self, delta: i32) {
        self.preview_scroll = (self.preview_scroll as i32 + delta).max(0) as u16;
        if self.editor.is_some() || self.any_popup() {
            return;
        }
    }

    fn reload(&mut self) {
        let keep = self.selected_note().map(|n| n.path.clone());
        let fresh = App::new(&self.vault);
        self.notes = fresh.notes;
        self.stamp = fresh.stamp;
        if self.sort_modified {
            self.notes.sort_by(|a, b| b.modified.cmp(&a.modified));
        }
        let idx = keep
            .and_then(|p| self.notes.iter().position(|n| n.path == p))
            .or(if self.notes.is_empty() { None } else { Some(0) });
        self.list_state.select(idx);
        self.history.clear();
        self.forward.clear();
        self.reset_link_selection();
    }

    fn refresh_after_edit(&mut self) {
        let hist: Vec<PathBuf> = self
            .history
            .iter()
            .filter_map(|&i| self.notes.get(i))
            .map(|n| n.path.clone())
            .collect();
        let fwd: Vec<PathBuf> = self
            .forward
            .iter()
            .filter_map(|&i| self.notes.get(i))
            .map(|n| n.path.clone())
            .collect();
        self.reload();
        self.history = hist
            .iter()
            .filter_map(|p| self.notes.iter().position(|n| n.path == *p))
            .collect();
        self.forward = fwd
            .iter()
            .filter_map(|p| self.notes.iter().position(|n| n.path == *p))
            .collect();
    }

    fn current_dir(&self) -> PathBuf {
        self.selected_note()
            .and_then(|n| n.path.parent().map(|p| p.to_path_buf()))
            .unwrap_or_else(|| self.vault.clone())
    }

    fn create_note(&mut self, name: &str) {
        let dir = self.current_dir();
        self.create_note_in(&dir, name);
    }

    fn create_note_in(&mut self, dir: &Path, name: &str) {
        let clean: String = name
            .trim()
            .chars()
            .filter(|c| !"\\/:*?\"<>".contains(*c))
            .collect();
        if clean.is_empty() {
            return ;
        }
        let _ = fs::create_dir_all(dir);
        let path = dir.join(format!("{clean}.md"));
        let is_new = !path.exists();
        if is_new {
            let _ = fs::write(&path, format!("# {clean}\n\n"));
        }
        self.refresh_after_edit();
        if let Some(idx) = self.notes.iter().position(|n| n.path == path) {
            self.list_state.select(Some(idx));
            self.reset_link_selection();
        }
        if is_new {
            self.status = format!("Created {clean}");
            self.start_editing();
        } else {
            self.status = format!("Opened existing note {clean}");
        }
    }

    fn open_daily(&mut self) {
        let today = chrono::Local::now().format("%Y-%m-%d").to_string();
        self.create_note(&today);
    }

    fn start_editing(&mut self) {
        let Some(n) = self.selected_note() else { return };
        let mut lines: Vec<String> = n.content.lines().map(|l| l.to_string()).collect();
        if lines.is_empty() {
            lines.push(String::new());
        }
        self.editor = Some(TextArea::new(lines));
        self.editor_dirty = false;
        self.discard_warned = false;
    }

    fn save_editor(&mut self) {
        let Some(ta) = &self.editor else { return };
        let text = format!("{}\n", ta.lines().join("\n"));
        let Some(idx) = self.list_state.selected() else { return };
        let path = self.notes[idx].path.clone();
        if fs::write(&path, &text).is_ok() {
            self.notes[idx].links = parse_links(&text);
            self.notes[idx].content =text;
            self.editor_dirty = false;
            self.discard_warned = false;
            self.stamp = vault_stamp(&self.vault);
            self.status = "Saved".to_string();
        }
    }

    fn handle_editor_key(&mut self, key: KeyEvent) {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);

        if self.ac_open {
            match key.code {
                KeyCode::Esc => {
                    self.ac_open = false;
                    return;
                }
                KeyCode::Enter | KeyCode::Tab => {
                    self.accept_autocomplete();
                    return;
                }
                KeyCode::Down => {
                    self.ac_state.select_next();
                    return;
                }
                KeyCode::Up => {
                    self.ac_state.select_previous();
                    return;
                }
                _ => {}
            }
        }

        match key.code {
            KeyCode::Char('s') if ctrl => self.save_editor(),
            KeyCode::Esc => {
                if self.editor_dirty && !self.discard_warned {
                    self.discard_warned = true;
                } else {
                    self.editor = None;
                    self.editor_dirty = false;
                    self.discard_warned = false;
                    self.reset_link_selection();
                }
            }
            _ => {
                if let Some(ta) = self.editor.as_mut() {
                    if ta.input(key) {
                        self.editor_dirty = true;
                        self.discard_warned = false;
                    }
                }
            }
        }

        self.update_autocomplete();
    }

    fn update_autocomplete(&mut self) {
        let q = self.editor.as_ref().and_then(|ta| ac_query(ta));
        match q {
            Some(q) => {
                let ql = q.to_lowercase();
                let mut m: Vec<usize> = self
                    .notes
                    .iter()
                    .enumerate()
                    .filter(|(_, n)| fuzzy(&q, &n.name))
                    .map(|(i, _)| i)
                    .collect();
                m.sort_by_key(|&i| {
                    let n = self.notes[i].name.to_lowercase();
                    (!n.starts_with(&ql), n.len())
                });
                m.truncate(8);
                self.ac_query = q;
                self.ac_open = !m.is_empty();
                self.ac_state.select(if m.is_empty() { None } else { Some(0) });
                self.ac_matches = m;
            }
            None => self.ac_open = false,
        }
    }

    fn accept_autocomplete(&mut self) {
        let picked = self
            .ac_state
            .selected()
            .and_then(|i| self.ac_matches.get(i).copied());
        let Some(idx) = picked else {
            self.ac_open = false;
            return;
        };
        let name = self.notes[idx].name.clone();
        let qlen = self.ac_query.chars().count();
        if let Some(ta) = self.editor.as_mut() {
            for _ in 0..qlen {
                ta.delete_char();
            }
            ta.insert_str(&name);
            let (row, col) = ta.cursor();
            let rest: String = ta.lines()[row].chars().skip(col).take(2).collect();
            if rest == "]]" {
                ta.move_cursor(CursorMove::Forward);
                ta.move_cursor(CursorMove::Forward);
            } else {
                ta.insert_str("]]");
            }
            self.editor_dirty = true;
            self.discard_warned = false;
        }
        self.ac_open = false;
    }

    fn open_search(&mut self) {
        self.search_open = true;
        self.search_query.clear();
        self.update_search();
    }

    fn update_search(&mut self) {
        self.search_results.clear();
        let q = self.search_query.to_lowercase();
        if q.chars().count() >= 2 {
            'outer: for (i, n) in self.notes.iter().enumerate() {
                for (ln, line) in n.content.lines().enumerate() {
                    if line.to_lowercase().contains(&q ) {
                        self.search_results.push((i, ln, line.trim().to_string()));
                        if self.search_results.len() >= 200 {
                            break 'outer;
                        }
                    }
                }
            }
        }
        self.search_state.select(if self.search_results.is_empty() {
            None
        } else {
            Some(0)
        });
    }

    fn handle_search_key(&mut self, code: KeyCode) {
        match code {
            KeyCode::Esc => self.search_open = false,
            KeyCode::Enter => {
                let picked = self
                    .search_state
                    .selected()
                    .and_then(|i| self.search_results.get(i))
                    .map(|(idx, ln, _)| (*idx, *ln));
                if let Some((idx, ln)) = picked {
                    self.jump_to(idx);
                    let prefix: String = self.notes[idx]
                        .content
                        .lines()
                        .take(ln)
                        .collect::<Vec<_>>()
                        .join("\n");
                    self.preview_scroll = render_markdown(&prefix, 0).len() as u16;
                }
                self.search_open = false;
            }
            KeyCode::Down => self.search_state.select_next(),
            KeyCode::Up => self.search_state.select_previous(),
            KeyCode::Backspace => {
                self.search_query.pop();
                self.update_search();
            }
            KeyCode::Char(c) => {
                self.search_query.push(c);
                self.update_search();
            }
            _ => {}
        }
    }

    fn open_tags(&mut self) {
        use std::collections::BTreeMap;
        let mut map: BTreeMap<String, Vec<usize>> = BTreeMap::new();
        for (i, n) in self.notes.iter().enumerate() {
            for t in parse_tags(&n.content) {
                map.entry(t).or_default().push(i);
            }
        }
        self.tag_list = map.into_iter().collect();
        self.tag_stage = None;
        self.tag_open = true;
        self.tag_state
            .select(if self.tag_list.is_empty() { None } else { Some(0) });
    }

    fn handle_tag_key(&mut self, code: KeyCode) {
        match code {
            KeyCode::Esc => {
                if self.tag_stage.is_some() {
                    let t = self.tag_stage.take();
                    self.tag_state.select(t);
                } else {
                    self.tag_open = false;
                }
            }
            KeyCode::Down | KeyCode::Char('j') => self.tag_state.select_next(),
            KeyCode::Up | KeyCode::Char('k') => self.tag_state.select_previous(),
            KeyCode::Enter => {
                let Some(sel) = self.tag_state.selected() else { return };
                match self.tag_stage {
                    None => {
                        self.tag_stage = Some(sel);
                        self.tag_state.select(Some(0));
                    }
                    Some(t) => {
                        let idx = self
                            .tag_list
                            .get(t)
                            .and_then(|(_, v)| v.get(sel))
                            .copied();
                        if let Some(idx) = idx {
                            if let Some(cur) = self.list_state.selected() {
                                self.history.push(cur);
                            }
                            self.forward.clear();
                            self.list_state.select(Some(idx));
                            self.reset_link_selection();
                        }
                        self.tag_open = false;
                        self.tag_stage = None;
                    }
                }
            }
            _ => {}
        }
    }

    fn build_adjency(&self) -> Vec<Vec<usize>> {
        use std::collections::HashMap;
        let mut by_name: HashMap<String, usize> = HashMap::new();
        for (i, n) in self.notes.iter().enumerate() {
            by_name.insert(n.name.to_lowercase(), i);
        }
        let mut adj: Vec<Vec<usize>> = vec![Vec::new(); self.notes.len()];
        for (i, n) in self.notes.iter().enumerate() {
            for l in &n.links {
                if let Some(&j) = by_name.get(l) {
                    if j != i {
                        adj[i].push(j);
                        adj[j].push(i);
                    }
                }
            }
        }
        for v in adj.iter_mut() {
            v.sort();
            v.dedup();
        }
        adj
    }

    fn open_graph(&mut self) {
        let Some(sel) = self.list_state.selected() else { return };
        self.graph_adj = self.build_adjency();
        self.graph_center = sel;
        self.graph_depth = 2;
        self.graph_sel = 0;
        self.graph_open = true;
    }

    fn graph_visible(&self) -> Vec<(usize, u8)> {
        let c = self.graph_center;
        let mut out = vec![(c, 0u8)];
        let Some(first) = self.graph_adj.get(c) else { return out };
        let ring1: Vec<usize> = first.iter().copied().take(40).collect();
        for &n in &ring1 {
            out.push((n, 1));
        }
        if self.graph_depth >= 2 {
            for &n in &ring1 {
                for &m in &self.graph_adj[n] {
                    if out.len() < 70 && !out.iter().any(|&(x, _)| x == m) {
                        out.push((m, 2));
                    }
                }
            }
        }
        out
    }

    fn handle_graph_key(&mut self, code: KeyCode) {
        let count = self.graph_visible().len().max(1);
        match code {
            KeyCode::Esc | KeyCode::Char('g') | KeyCode::Char('q') => self.graph_open = false,
            KeyCode::Right
            | KeyCode::Down
            | KeyCode::Tab
            | KeyCode::Char('j')
            | KeyCode::Char('l') => {
                self.graph_sel = (self.graph_sel + 1) % count;
            }
            KeyCode::Left
            | KeyCode::Up
            | KeyCode::Char('k')
            | KeyCode::Char('h') => {
                self.graph_sel = (self.graph_sel + count - 1) % count;
            }
            KeyCode::Char('1') => {
                self.graph_depth = 1;
                self.graph_sel = 0;
            }
            KeyCode::Char('2') => {
                self.graph_depth = 2;
                self.graph_sel = 0;
            }
            KeyCode::Enter => {
                let target = self.graph_visible().get(self.graph_sel).map(|&(i, _)| i);
                if let Some(idx) = target {
                    if self.list_state.selected() != Some(idx) {
                        if let Some(cur) = self.list_state.selected() {
                            self.history.push(cur);
                        }
                        self.forward.clear();
                        self.list_state.select(Some(idx));
                        self.reset_link_selection();
                    }
                    self.graph_center = idx;
                    self.graph_sel = 0;
                }
            }
            _ => {}
        }
    }

    fn jump_to(&mut self, idx: usize) {
        if self.list_state.selected() != Some(idx) {
            if let Some(cur) = self.list_state.selected() {
                self.history.push(cur);
            }
            self.forward.clear();
            self.list_state.select(Some(idx));
            self.reset_link_selection();
        }
    }

    fn any_popup(&self) -> bool {
        self.switcher_open
            || self.prompt_open
            || self.search_open
            || self.tag_open
            || self.graph_open
            || self.help_open
            || self.tasks_open
            || self.recent_open
            || self.health_open
            || self.outline_open
            || self.vault_open
            || self.confirm_delete
            || self.confirm_delete
    }

    fn tick(&mut self) {
        let cur = self.selected_note().map(|n| n.path.clone());
        if cur != self.dwell_path {
            self.dwell_path = cur;
            self.dwell_since = Instant::now();
        } else if let Some(p) = cur {
            if self.dwell_since.elapsed() >= Duration::from_millis(1500)
                && self.recent.first() != Some(&p)
            {
                self.recent.retain(|x| x != &p);
                self.recent.insert(0, p);
                self.recent.truncate(15);
            }
        }
        
        if self.last_check.elapsed() >= Duration::from_secs(2) {
            self.last_check = Instant::now();
            if self.editor.is_none() && !self.any_popup() {
                let now = vault_stamp(&self.vault);
                if now != self.stamp {
                    self.refresh_after_edit();
                    self.status = "Vault changed on disk - reloaded".to_string();
                }
            }
        }
    }

    fn open_trash(&mut self) {
        let mut items: Vec<(PathBuf, String)> = Vec::new();
        if let Ok(rd) = fs::read_dir(self.vault.join(".trash")) {
            for e in rd.flatten() {
                let p = e.path();
                if p.extension().map_or(false, |x| x == "md") {
                    let name = p.file_stem().unwrap_or_default().to_string_lossy().to_string();
                    items.push((p, name));
                }
            }
        }
        items.sort_by(|a, b| a.1.to_lowercase().cmp(&b.1.to_lowercase()));
        self.trash_state.select(if items.is_empty() { None } else { Some(0) });
        self.trash_items = items;
        self.trash_open = true;
    }

    fn restore_trashed(&mut self) {
        let Some(i) = self.trash_state.selected() else { return };
        let Some((from, name)) = self.trash_items.get(i).cloned() else { return };
        let to = self.vault.join(format!("{name}.md"));
        if to.exists() {
            self.status = format!("'{name}' already exists in vault");
            return;
        }
        match fs::rename(&from, &to) {
            Ok(_) => {
                self.status = format!("Restored '{name}'");
                self.trash_items.remove(i);
                self.trash_state.select(if self.trash_items.is_empty() {
                    None
                } else {
                    Some(i.min(self.trash_items.len() - 1))
                });
                self.refresh_after_edit();
            }
            Err(e) => self.status = format!("Restor failted: {e}"),
        }
    }
    
    fn handle_trash_key(&mut self, code: KeyCode) {
        match code {
            KeyCode::Esc | KeyCode::Char('q') => self.trash_open = false,
            KeyCode::Down | KeyCode::Char('j') => self.trash_state.select_next(),
            KeyCode::Up | KeyCode::Char('k') => self.trash_state.select_previous(),
            KeyCode::Enter => self.restore_trashed(),
            _ => {}
        }
    }

    fn delete_selected(&mut self) {
        let Some(idx) = self.list_state.selected() else { return };
        let path = self.notes[idx].path.clone();
        let name = self.notes[idx].name.clone();
        let trash = self.vault.join(".trash");
        if fs::create_dir_all(&trash).is_err() {
            self.status = "Could not create the .trash folder".to_string();
            return;
        }
        let mut dest = trash.join(format!("{name}.md"));
        let mut n = 1;
        while dest.exists() {
            dest = trash.join(format!("{name} ({n}).md"));
            n += 1;
        }
        match fs::rename(&path, &dest) {
            Ok(_) => {
                self.refresh_after_edit();
                self.status = format!("Moved '{name}' to .trash");
            }
            Err(e) => self.status = format!("Delete failed: {e}"),
        }
    }

    fn rename_selected(&mut self, new_name: &str) {
        let Some(idx) = self.list_state.selected() else { return };
        let clean: String = new_name
            .trim()
            .chars()
            .filter(|c| !"\\/:*?\"<>|".contains(*c))
            .collect();
        let old_name = self.notes[idx].name.clone();
        if clean.is_empty() || clean == old_name {
            return;
        }
        let old_path = self.notes[idx].path.clone();
        let new_path = old_path.with_file_name(format!("{clean}.md"));
        if new_path.exists() && clean.to_lowercase() != old_name.to_lowercase() {
            self.status = format!("'{clean}' already exists");
            return;
        }

        let to_fix: Vec<PathBuf> = self
            .notes
            .iter()
            .filter(|n| n.links.iter().any(|l| resolve_link(&self.notes, l) == Some(idx)))
            .map(|n| {
                if n.path == old_path {
                    new_path.clone()
                } else {
                    n.path.clone()
                }
            })
            .collect();

        if let Err(e) = fs::rename(&old_path, &new_path) {
            self.status = format!("Rename failed: {e}");
            return;
        }

        let pattern = format!(r"\[\[(\s*[^\]|#]*/)?\s*{}\s*([|#]|\]\])", regex::escape(&old_name));
        let re = regex::RegexBuilder::new(&pattern)
            .case_insensitive(true)
            .build()
            .unwrap();
        let mut updated = 0;
        for p in &to_fix {
            if let Ok(text) = fs::read_to_string(p) {
                let new_text = re
                    .replace_all(&text, |caps: &regex::Captures| {
                        let prefix = caps.get(1).map_or("", |m| m.as_str());
                        format!("[[{}{}", clean, &caps[1])
                    })
                    .into_owned();
                if new_text != text && fs::write(p, &new_text).is_ok() {
                    updated += 1;
                }
            }
        }

        self.refresh_after_edit();
        let lower = clean.to_lowercase();
        if let Some(i) = self
            .notes
            .iter()
            .position(|n| n.name.to_lowercase() == lower)
        {
        self.list_state.select(Some(i));
        self.reset_link_selection();
        }
        self.status = format!("Renamed '{old_name}' to '{clean}' ({updated} notes updated)");
    }

    fn collect_tasks(&mut self) {
        self.tasks.clear();
        for(i, n) in self.notes.iter().enumerate() {
            let mut in_code = false;
            for (ln, line) in n.content.lines().enumerate() {
                if line.trim_start().starts_with("```") {
                    in_code = !in_code;
                    continue;
                }
                if in_code {
                    continue;
                }
                if let Some((done, text)) = parse_task(line.trim_start()) {
                    if done && !self.tasks_show_done {
                        continue;
                    }
                    self.tasks.push(TaskItem {
                        note: i,
                        line: ln,
                        done,
                        text: text.to_string(),
                    });
                }
            }
        }
        let sel = self
            .task_state
            .selected()
            .unwrap_or(0)
            .min(self.tasks.len().saturating_sub(1));
        self.task_state
            .select(if self.tasks.is_empty() { None } else { Some(sel) });
    }

    fn open_tasks(&mut self) {
        self.tasks_open = true;
        self.task_state.select(Some(0));
        self.collect_tasks();
    }

    fn toggle_task(&mut self) {
        let Some(sel) = self.task_state.selected() else { return };
        let Some(t) = self.tasks.get(sel) else { return };
        let (ni, ln) = (t.note, t.line);
        let path = self.notes[ni].path.clone();
        let mut out: Vec<String> = self.notes[ni]
            .content
            .lines()
            .map(|l| l.to_string())
            .collect();
        if let Some(line) = out.get_mut(ln) {
            if line.contains("[ ]") {
                *line = line.replacen("[ ]", "[x]", 1);
            } else if line.contains("[x]") {
                *line = line.replacen("[x]", "[ ]", 1);
            } else if line.contains("[X]") {
                *line = line.replacen("[X]", "[ ]", 1);
            }
        }
        let new = format!("{}\n", out.join("\n"));
        if fs::write(&path, &new).is_ok() {
            self.notes[ni].links = parse_links(&new);
            self.notes[ni].content = new;
            self.stamp = vault_stamp(&self.vault);
            self.status = "Task updated".to_string();
        }
        self.collect_tasks();
    }

    fn handle_tasks_key(&mut self, code: KeyCode) {
        match code {
            KeyCode::Esc | KeyCode::Char('q') => self.tasks_open = false,
            KeyCode::Down | KeyCode::Char('j') => self.task_state.select_next(),
            KeyCode::Up | KeyCode::Char('k') => self.task_state.select_previous(),
            KeyCode::Char(' ') | KeyCode::Char('x') => self.toggle_task(),
            KeyCode::Char('a') => {
                self.tasks_show_done = !self.tasks_show_done;
                self.collect_tasks();
            }
            KeyCode::Enter => {
                let target = self
                    .task_state
                    .selected()
                    .and_then(|i| self.tasks.get(i))
                    .map(|t| t.note);
                if let Some(idx) = target {
                    self.jump_to(idx);
                }
                self.tasks_open = false;
            }
            _ => {}
        }
    }

    fn open_outline(&mut self) {
        let Some(n) = self.selected_note() else { return };
        let mut items: Vec<(usize, usize, String)> = Vec::new();
        let mut in_code = false;
        let mut in_front = false;
        for (ln, raw) in n.content.lines().enumerate() {
            if ln == 0 && raw.trim() == "---" {
                in_front = true;
                continue;
            }
            if in_front {
                if raw.trim() == "---" {
                    in_front = false;
                }
                continue;
            }
            if raw.trim_start().starts_with("```"){
                in_code = !in_code;
                continue;
            }
            if in_code {
                continue;
            }
            let trimmed = raw.trim_start();
            let level = trimmed.chars().take_while(|&c| c == '#').count();
            if (1..=6).contains(&level) && trimmed.chars().nth(level) == Some(' ') {
                items.push((level, ln, trimmed[level + 1..].trim().to_string()));
            }
        }
        self.outline_state
            .select(if items.is_empty() { None } else { Some(0) });
        self.outline_items = items;
        self.outline_open = true;
    }

    fn handle_outline_key(&mut self, code: KeyCode) {
        match code {
            KeyCode::Esc | KeyCode::Char('q') => self.outline_open = false,
            KeyCode::Down | KeyCode::Char('j') => self.outline_state.select_next(),
            KeyCode::Up | KeyCode::Char('k') => self.outline_state.select_previous(),
            KeyCode::Enter => {
                let target = self
                    .outline_state
                    .selected()
                    .and_then(|i| self.outline_items.get(i))
                    .map(|(_, ln, _)| *ln);
                if let (Some(ln), Some(idx)) = (target, self.list_state.selected()) {
                    let prefix: String = self.notes[idx]
                        .content
                        .lines()
                        .take(ln)
                        .collect::<Vec<_>>()
                        .join("\n");
                    self.preview_scroll = render_markdown(&prefix, 0).len() as u16;
                }
                self.outline_open = false;
            }
            _ => {}
        }
    }

    fn open_vaults(&mut self) {
        self.vault_items = known_vaults(&self.vault);
        self.vault_state.select(Some(0));
        self.vault_open = true;
    }

    fn handle_vault_key(&mut self, code: KeyCode) {
        match code {
            KeyCode::Esc | KeyCode::Char('q') => self.vault_open = false,
            KeyCode::Down | KeyCode::Char('j') => self.vault_state.select_next(),
            KeyCode::Up | KeyCode::Char('k') => self.vault_state.select_previous(),
            KeyCode::Enter => {
                let target = self
                    .vault_state
                    .selected()
                    .and_then(|i| self.vault_items.get(i).cloned());
                self.vault_open = false;
                if let Some(p) = target {
                    if p != self.vault {
                        *self = App::new(&p);
                        let name = p
                            .file_name()
                            .map(|s| s.to_string_lossy().to_string())
                            .unwrap_or_else(|| p.display().to_string());
                        self.status = format!("Opened vault {name}");
                    }
                }
            }
            _ => {}
        }
    }

    fn open_recent(&mut self) {
        let current = self.list_state.selected();
        self.recent_items = self
            .recent
            .iter()
            .filter_map(|p| self.notes.iter().position(|n| n.path == *p))
            .filter(|&i| Some(i) != current)
            .collect();
        self.recent_state.select(if self.recent_items.is_empty() {
            None
        } else {
            Some(0)
        });
        self.recent_open = true;
    }

    fn open_health(&mut self) {
        use std::collections::HashSet;
        let mut names: HashSet<String> = HashSet::new();
        for (i, n) in self.notes.iter().enumerate() {
            names.insert(n.name.to_lowercase());
            names.insert(n.rel.to_lowercase());
        }
        
        let mut linked_to: HashSet<usize> = HashSet::new();
        for (i, n) in self.notes.iter().enumerate() {
            for l in &n.links {
                if let Some(t) = resolve_link(&self.notes, l) {
                    if t != i {
                        linked_to.insert(t);
                    }
                }
            }
        }

        let mut items: Vec<(usize, String)> = Vec::new();

        for (i, n) in self.notes.iter().enumerate() {
            let mut seen: HashSet<&String> = HashSet::new();
            for l in &n.links {
                if !names.contains(l) && seen.insert(l) {
                    items.push((i, format!("missing [[{l}]] in {}", n.name)));
                }
            }
        }

        for (i, n) in self.notes.iter().enumerate() {
            let has_out = n.links.iter().any(|l| resolve_link(&self.notes, l).map_or(false, |t| t != i));
            let has_in = linked_to.contains(&i);
            if !has_out && !has_in {
                items.push((i, format!("orphan {}", n.name)));
            }
        }

        let wiki_re = Regex::new(r"\[\[[^\]]*\]\]").unwrap();
        for (i, n) in self.notes.iter().enumerate() {
            let plain = wiki_re.replace_all(&n.content, "").to_string();
            for (j, target) in self.notes.iter().enumerate() {
                if i == j || target.name.len() < 3 {
                    continue;
                }
                let re = regex::RegexBuilder::new(&format!(r"\b{}\b", regex::escape(&target.name)))
                    .case_insensitive(true)
                    .build()
                    .unwrap();
                if re.is_match(&plain) {
                    items.push((i, format!("unlinked '{}' in {}", target.name, n.name)));
                }
            }
        }

        self.health_state
            .select(if items.is_empty() { None } else { Some(0) });
        self.health_items = items;
        self.health_open = true;
    }

    fn handle_health_key(&mut self, code: KeyCode) {
        match code {
            KeyCode::Esc | KeyCode::Char('q') => self.health_open = false,
            KeyCode::Down | KeyCode::Char('j') => self.health_state.select_next(),
            KeyCode::Up | KeyCode::Char('k') => self.health_state.select_previous(),
            KeyCode::Enter => {
                let target = self
                    .health_state
                    .selected()
                    .and_then(|i| self.health_items.get(i))
                    .map(|(idx, _)| *idx);
                if let Some(idx) = target {
                    self.jump_to(idx);
                }
                self.health_open = false;
            }
            _ => {}
        }
    }

    fn handle_recent_key(&mut self, code: KeyCode) {
        match code {
            KeyCode::Esc | KeyCode::Char('q') => self.recent_open = false,
            KeyCode::Down | KeyCode::Char('j') => self.recent_state.select_next(),
            KeyCode::Up | KeyCode::Char('k') => self.recent_state.select_previous(),
            KeyCode::Enter => {
                let target = self
                    .recent_state
                    .selected()
                    .and_then(|i| self.recent_items.get(i).copied());
                if let Some(idx) = target {
                    self.jump_to(idx);
                }
                self.recent_open = false;
            }
            _ => {}
        }
    }

    fn handle_prompt_key(&mut self, code: KeyCode) {
        match code {
            KeyCode::Esc => self.prompt_open = false,
            KeyCode::Enter => {
                let name = std::mem::take(&mut self.prompt_text);
                self.prompt_open = false;
                if self.prompt_rename {
                    self.rename_selected(&name);
                } else {
                    self.create_note(&name);
                }
            }
            KeyCode::Backspace => {
                self.prompt_text.pop();
            }
            KeyCode::Char(c) => self.prompt_text.push(c),
            _ => {}
        }
    }

    fn open_switcher(&mut self) {
        self.switcher_open = true;
        self.query.clear();
        self.update_matches();
    }

    fn update_matches(&mut self) {
        self.matches = self
            .notes
            .iter()
            .enumerate()
            .filter(|(_, n)| fuzzy(&self.query, &n.name))
            .map(|(i, _)| i)
            .collect();
        self.switcher_state
            .select(if self.matches.is_empty() { None } else { Some(0) });
    }

    fn handle_switcher_key(&mut self, code: KeyCode) {
        match code {
            KeyCode::Esc => self.switcher_open = false,
            KeyCode::Enter => {
                let picked = self
                    .switcher_state
                    .selected()
                    .and_then(|i| self.matches.get(i).copied());
                if let Some(idx) = picked {
                    if let Some(cur) = self.list_state.selected() {
                        self.history.push(cur);
                    }
                    self.forward.clear();
                    self.list_state.select(Some(idx));
                    self.reset_link_selection();
                }
                self.switcher_open = false;
            }
            KeyCode::Down => self.switcher_state.select_next(),
            KeyCode::Up => self.switcher_state.select_previous(),
            KeyCode::Backspace => {
                self.query.pop();
                self.update_matches();
            }
            KeyCode::Char(c) => {
                self.query.push(c);
                self.update_matches();
            }
            _ => {}
        }
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
        let Some(target) = note.links.get(i).cloned() else { return };
        let original = link_original(&note.content, &target).unwrap_or_else(|| target.clone());
        if let Some(idx) = resolve_link(&self.notes, &target) {
            self.jump_to(idx);
            if let Some((_, h)) = target.split_once('#') {
                let wanted = h.trim().to_lowercase();
                if let Some(ln) = heading_line(&self.notes[idx].content, &wanted) {
                    let prefix: String = self.notes[idx]
                        .content
                        .lines()
                        .take(ln)
                        .collect::<Vec<_>>()
                        .join("\n");
                    self.preview_scroll = render_markdown(&prefix, 0).len() as u16;
                }
            }
            return;
        }
        if let Some(cur) = self.list_state.selected() {
            self.history.push(cur);
        }
        self.forward.clear();
        let clean = original.split('#').next().unwrap_or(&original)
            .trim().trim_start_matches("./").replace('\\', "/");
        let (dir, name) = match clean.rsplit_once('/') {
            Some((parent, leaf)) => (self.vault.join(parent), leaf.to_string()),
            None => (self.current_dir(), clean.clone()),
        };
        self.create_note_in(&dir, &name);
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
        if self.editor.is_some() || self.any_popup() {
            return;
        }
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
        let Some(sel) = self.list_state.selected() else {
            return Vec::new();
        };
        self.notes
            .iter()
            .enumerate()
            .filter(|(i, n)| {
                *i != sel && n.links.iter().any(|l| resolve_link(&self.notes, l) == Some(sel))
            })
            .map(|(_, n)| n)
            .collect()
    }

    fn handle_key(&mut self, code: KeyCode) {
        if self.switcher_open {
            self.handle_switcher_key(code);
            return;
        }
        if self.prompt_open {
            self.handle_prompt_key(code);
            return;
        }
        if self.search_open {
            self.handle_search_key(code);
            return;
        }
        if self.tag_open {
            self.handle_tag_key(code);
            return;
        }
        if self.graph_open {
            self.handle_graph_key(code);
            return;
        }
        if self.help_open {
            match code {
                KeyCode::Down | KeyCode::Char('j') => {
                    self.help_scroll = self.help_scroll.saturating_add(1)
                }
                KeyCode::Up | KeyCode::Char('k') => {
                    self.help_scroll = self.help_scroll.saturating_sub(1)
                }
                KeyCode::PageDown => self.help_scroll = self.help_scroll.saturating_add(1),
                KeyCode::PageUp => self.help_scroll = self.help_scroll.saturating_sub(1),
                _ => self.help_open = false,
            }
            return;
        }
        if self.confirm_delete {
            self.confirm_delete = false;
            if code == KeyCode::Char('y') {
                self.delete_selected();
            }
            return;
        }
        if self.health_open {
            self.handle_health_key(code);
            return;
        }
        if self.outline_open {
            self.handle_outline_key(code);
            return;
        }
        if self.vault_open {
            self.handle_vault_key(code);
            return;
        }
        if self.tasks_open {
            self.handle_tasks_key(code);
            return;
        }
        if self.recent_open {
            self.handle_recent_key(code);
            return;
        }
        if self.trash_open {
            self.handle_trash_key(code);
            return;
        }
        let code = match code {
            KeyCode::Char(c) => KeyCode::Char(*self.keymap.get(&c).unwrap_or(&c)),
            other => other,
        };
        match code {
            KeyCode::Char('q') | KeyCode::Esc => self.should_quit = true,
            KeyCode::Tab | KeyCode::BackTab => {
                self.focus = match self.focus {
                    Focus::Notes => Focus::Links,
                    Focus::Links => Focus::Preview,
                    Focus::Preview => Focus::Notes,
                };
            }
            KeyCode::Down | KeyCode::Char('j') => match self.focus {
                Focus::Notes => self.move_note_selection(1),
                Focus::Links => self.link_state.select_next(),
                Focus::Preview => self.move_view_cursor(1),
            }, 
            KeyCode::Up | KeyCode::Char('k') => match self.focus {
                Focus::Notes => self.move_note_selection(-1),
                Focus::Links => self.link_state.select_previous(),
                Focus::Preview => self.move_view_cursor(-1),
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
            KeyCode::Char('r') => self.reload(),
            KeyCode::Char('e') => self.start_editing(),
            KeyCode::Char('E') => self.edit_requested = true,
            KeyCode::Char('n') => {
                self.prompt_text.clear();
                self.prompt_rename = false;
                self.prompt_open = true;
            }
            KeyCode::Char('d') => self.open_daily(),
            KeyCode::Char('/') => self.open_search(),
            KeyCode::Char('t') => self.open_tags(),
            KeyCode::Char('g') => self.open_graph(),
            KeyCode::Char('?') => {
                self.help_scroll = 0;
                self.help_open = true;
            }
            KeyCode::Char('D') => {
                if self.selected_note().is_some() {
                    self.confirm_delete = true;
                }
            }
            KeyCode::Char('R') => {
                if let Some(name) = self.selected_note().map(|n| n.name.clone()) {
                    self.prompt_text = name;
                    self.prompt_rename = true;
                    self.prompt_open = true;
                }
            }
            KeyCode::Char('T') => self.open_tasks(),
            KeyCode::Char('o') => self.open_recent(),
            KeyCode::Char('u') => self.open_health(),
            KeyCode::Char('X') => self.open_trash(),
            KeyCode::Char('s') => {
                self.sort_modified = !self.sort_modified;
                self.refresh_after_edit();
                self.status = if self.sort_modified {
                    "Sorted by last modified".to_string()
                } else {
                    "Sorted by name".to_string()
                };
            }
            KeyCode::Char('O') => self.open_outline(),
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

fn fuzzy(query: &str, name: &str) -> bool {
    let lower = name.to_lowercase();
    let mut chars = lower.chars();
    query
        .to_lowercase()
        .chars()
        .all(|qc| chars.any(|c| c == qc))
}

fn find_close(chars: &[char], from: usize, marker: &[char]) -> Option<usize> {
    let n = marker.len();
    if chars.len() < n {
        return None;
    }
    (from..=chars.len() - n).find(|&j| chars[j..j + n] == *marker)
}

fn push_buf(spans: &mut Vec<Span<'static>>, buf: &mut String, base: Style) {
    if !buf.is_empty() {
        spans.push(Span::styled(std::mem::take(buf), base));
    }
}

fn inline_spans(text: &str, base: Style) -> Vec<Span<'static>> {
    let chars: Vec<char> = text.chars().collect();
    let mut spans: Vec<Span<'static>> = Vec::new();
    let mut buf = String::new();
    let mut i = 0;

    while i < chars.len() {
        let c = chars[i];

        // [[wikilink]]
        if c == '[' && chars.get(i + 1) == Some(&'[') {
            if let Some(end) = find_close(&chars, i + 2, &[']', ']']) {
                push_buf(&mut spans, &mut buf, base);
                let s: String = chars[i..end + 2].iter().collect();
                spans.push(Span::styled(s, base.fg(Color::Yellow)));
                i = end + 2;
                continue;
            }
        }

        // **bold**
        if c == '*' && chars.get(i + 1) == Some(&'*') {
            if let Some(end) = find_close(&chars, i + 2, &['*', '*']) {
                if end > i + 2 {
                    push_buf(&mut spans, &mut buf, base);
                    let s: String = chars[i + 2..end].iter().collect();
                    spans.push(Span::styled(s, base.add_modifier(Modifier::BOLD)));
                    i = end + 2;
                    continue;
                }
            }
        }

        // *italic*
        if c == '*'
            && chars
                .get(i + 1)
                .map_or(false, |n| !n.is_whitespace() && *n != '*')
        {
            if let Some(end) = find_close(&chars, i + 1, &['*']) {
                push_buf(&mut spans, &mut buf, base);
                let s: String = chars[i + 1..end].iter().collect();
                spans.push(Span::styled(s, base.add_modifier(Modifier::ITALIC)));
                i = end + 1;
                continue;
            }
        }

        // `inline code`
        if c == '`' {
            if let Some(end) = find_close(&chars, i + 1, &['`']) {
                push_buf(&mut spans, &mut buf, base);
                let s: String = chars[i + 1..end].iter().collect();
                spans.push(Span::styled(s, base.fg(Color::Green)));
                i = end + 1;
                continue;
            }
        }

        buf.push(c);
        i += 1;
    }
    push_buf(&mut spans, &mut buf, base);
    spans
}

fn render_markdown(content: &str, width: usize) -> Vec<Line<'static>> {
    let dim = Style::new().fg(Color::DarkGray);
    let mut lines: Vec<Line<'static>> = Vec::new();
    let mut in_code = false;
    let mut in_front = false;

    for (i, raw) in content.lines().enumerate() {
        // YAML frontmatter at the top of the file
        if i == 0 && raw.trim() == "---" {
            in_front = true;
            lines.push(Line::styled(raw.to_string(), dim));
            continue;
        }
        if in_front {
            lines.push(Line::styled(raw.to_string(), dim));
            if raw.trim() == "---" {
                in_front = false;
            }
            continue;
        }

        // Fenced code blocks
        if raw.trim_start().starts_with("```") {
            in_code = !in_code;
            lines.push(Line::styled(raw.to_string(), dim));
            continue;
        }
        if in_code {
            lines.push(Line::styled(raw.to_string(), Style::new().fg(Color::Green)));
            continue;
        }

        let trimmed = raw.trim_start();
        let indent = &raw[..raw.len() - trimmed.len()];

        // Horizontal rule (***, ---, ___)
        let t = trimmed.trim();
        if t.len() >= 3
            && (t.chars().all(|c| c == '*')
                || t.chars().all(|c| c == '-')
                || t.chars().all(|c| c == '_'))
        {
            lines.push(Line::styled("─".repeat(width), dim));
            continue;
        }

        // Headings
        let level = trimmed.chars().take_while(|&c| c == '#').count();
        if (1..=6).contains(&level) && trimmed.chars().nth(level) == Some(' ') {
            let text = trimmed[level + 1..].trim().to_string();
            match level {
                1 => {
                    let style = Style::new().fg(Color::Cyan).add_modifier(Modifier::BOLD);
                    lines.push(Line::styled(text.to_uppercase(), style));
                    lines.push(Line::styled("═".repeat(width), Style::new().fg(Color::Cyan)));
                }
                2 => {
                    let style = Style::new().fg(Color::Blue).add_modifier(Modifier::BOLD);
                    let underline = "─".repeat(text.chars().count());
                    lines.push(Line::styled(text, style));
                    lines.push(Line::styled(underline, Style::new().fg(Color::Blue)));
                }
                3 => {
                    let style = Style::new().fg(Color::Magenta).add_modifier(Modifier::BOLD);
                    lines.push(Line::styled(format!("▌ {text}"), style));
                }
                _ => {
                    let style = Style::new().fg(Color::Magenta).add_modifier(Modifier::ITALIC);
                    lines.push(Line::styled(text, style));
                }
            }
            continue;
        }

        // Blockquotes (nested with > >)
        if trimmed.starts_with('>') {
            let mut depth = 0;
            let mut rest = trimmed;
            while let Some(r) = rest.strip_prefix('>') {
                depth += 1;
                rest = r.trim_start();
            }
            let qstyle = dim.add_modifier(Modifier::ITALIC);
            let mut spans = vec![Span::styled("| ".repeat(depth), dim)];
            spans.extend(inline_spans(rest, qstyle));
            lines.push(Line::from(spans));
            continue;
        }

        // Tasks: - [ ] and - [x]
        if let Some((done, text)) = parse_task(trimmed) {
            let (mark, mark_style) = if done {
                ("☑ ", Style::new().fg(Color::Green))
            } else {
                ("☐ ", Style::new().fg(Color::Yellow))
            };
            let body_style = if done {
                dim.add_modifier(Modifier::CROSSED_OUT)
            } else {
                Style::new()
            };
            let mut spans = vec![Span::raw(indent.to_string()), Span::styled(mark, mark_style)];
            spans.extend(inline_spans(text, body_style));
            lines.push(Line::from(spans));
            continue;
        }
        
        // Bullet lists (-, *, +), nested by indentation
        if trimmed.starts_with("- ") || trimmed.starts_with("* ") || trimmed.starts_with("+ ") {
            let depth = indent.len() / 2;
            let bullet = if depth % 2 == 0 { "•" } else { "◦" };
            let mut spans = vec![Span::raw(format!("{indent}{bullet}"))];
            spans.extend(inline_spans(&trimmed[2..], Style::new()));
            lines.push(Line::from(spans));
            continue;
        }

        // Ordered lists (1. 2. 3.)
        let digits = trimmed.chars().take_while(|c| c.is_ascii_digit()).count();
        if digits > 0 && trimmed[digits..].starts_with(". ") {
            let mut spans = vec![Span::styled(
                format!("{indent}{}", &trimmed[..digits + 2]),
                Style::new().fg(Color::Cyan),
            )];
            spans.extend(inline_spans(&trimmed[digits + 2..], Style::new()));
            lines.push(Line::from(spans));
            continue;
        }

        // Normal text
        lines.push(Line::from(inline_spans(raw, Style::new())));
    }
    lines
}

fn heading_line(content: &str, wanted: &str) -> Option<usize> {
    let mut in_code = false;
    let mut in_front = false;
    for (ln, raw) in content.lines().enumerate() {
        if ln == 0 && raw.trim() == "---" {
            in_front = true;
            continue;
        }
        if in_front {
            if raw.trim() == "---" {
                in_front = false;
            }
            continue;
        }
        if raw.trim_start().starts_with("```") {
            in_code = !in_code;
            continue;
        }
        if in_code {
            continue;
        }
        let trimmed = raw.trim_start();
        let level = trimmed.chars().take_while(|&c| c == '#').count();
        if (1..=6).contains(&level)
            && trimmed.chars().nth(level) == Some(' ')
            && trimmed[level + 1..].trim().to_lowercase() == wanted
        {
            return Some(ln);
        }
    }
    None
}

fn resolve_link(notes: &[Note], target: &str) -> Option<usize> {
    let t = target.split('#').next().unwrap_or(target)
        .trim().trim_start_matches("./").replace('\\', "/").to_lowercase();
    if let Some(i) = notes.iter().position(|n| n.rel.to_lowercase() == t) {
        return Some(i);
    }
    if t.contains('/') {
        return notes.iter().position(|n| n.rel.to_lowercase().ends_with(&format!("/{t}")));
    }
    notes.iter().position(|n| n.name.to_lowercase() == t)
}

fn parse_links(content: &str) -> Vec<String> {
    let re = Regex::new(r"\[\[([^\]|]+)(?:[|][^\]]*)?\]\]").unwrap();
    let links: Vec<String> = re
        .captures_iter(content)
        .map(|c| c[1].trim().to_lowercase())
        .collect();
    links
}

fn parse_tags(content: &str) -> Vec<String> {
    let re = Regex::new(r"(?m)(?:^|\s)#([A-Za-z][\w/-]*)").unwrap();
    let mut tags: Vec<String> = re
        .captures_iter(content)
        .map(|c| c[1].to_lowercase())
        .collect();
    if let Some(rest) = content.strip_prefix("---") {
        if let Some(end) = rest.find("\n---") {
            let front = &rest[..end];
            let mut in_tags = false;
            for line in front.lines() {
                let t = line.trim();
                if let Some(raw) = t.strip_prefix("tags:") {
                    in_tags = true;
                    let items = raw.trim()
                        .trim_start_matches('[')
                        .trim_end_matches(']');
                    for x in items.split(',') {
                        let tag = x.trim()
                            .trim_matches('\'') 
                            .trim_matches('"')
                            .trim_start_matches('#');
                        if !tag.is_empty() {
                            tags.push(tag.to_lowercase());
                        }
                    }
                    continue;
                }
                if in_tags {
                    if let Some(x) = t.strip_prefix("- ") {
                        let tag = x.trim()
                            .trim_matches('\'')
                            .trim_matches('"')
                            .trim_start_matches('#');
                        if !tag.is_empty() {
                            tags.push(tag.to_lowercase());
                        }
                    } else if !t.is_empty() {
                        in_tags = false;
                    }
                }
            }
        }
    }
    tags.sort();
    tags.dedup();
    tags
}

fn graph_layout(visible: &[(usize, u8)], rx: f64, ry: f64, r1: f64) -> Vec<(f64, f64)> {
    let n1 = visible.iter().filter(|(_, r)| *r == 1).count();
    let n2 = visible.iter().filter(|(_, r)| *r == 2).count();
    let (mut i1, mut i2) = (0usize, 0usize);
    visible
        .iter()
        .map(|&(_, ring)| match ring {
            0 => (0.0, 0.0),
            1 => {
                let a = std::f64::consts::TAU * i1 as f64 / n1 as f64
                    - std::f64::consts::FRAC_PI_2;
                i1 += 1;
                (a.cos() * rx * r1, a.sin() * ry * r1)
            }
            _ => {
                let a = std::f64::consts::TAU * i2 as f64 / n2 as f64
                    - std::f64::consts::FRAC_PI_2;
                i2 += 1;
                (a.cos() * rx, a.sin() * ry)
            }
        })
        .collect()
}

fn ac_query(ta: &TextArea) -> Option<String> {
    let(row, col) = ta.cursor();
    let line = ta.lines().get(row)?;
    let before: String = line.chars().take(col).collect();
    let pos = before.rfind("[[")?;
    let after = &before[pos + 2..];
    if after.contains(|c| c == ']' || c == '|' || c == '#') {
        return None;
    }
    Some(after.to_string())
}

fn parse_task(trimmed: &str) -> Option<(bool, &str)> {
    for p in ["- ", "* ", "+ "] {
        if let Some(rest) = trimmed.strip_prefix(p) {
            if let Some(t) = rest.strip_prefix("[ ] ") {
                return Some((false, t));
            }
            if let Some(t) = rest
                .strip_prefix("[x] ")
                .or_else(|| rest.strip_prefix("[X] "))
            {
                return Some((true, t));
            }
        }
    }
    None
}

fn link_original(content: &str, lower: &str) -> Option<String> {
    let re = Regex::new(r"\[\[([^\]|]+)(?:[|][^\]]*)?\]\]").unwrap();
    let found = re
        .captures_iter(content)
        .map(|c| c[1].trim().to_string())
        .find(|s| s.to_lowercase() == lower);
    found
}

fn vault_stamp(vault: &Path) -> (usize, u128) {
    let mut count = 0usize;
    let mut sum = 0u128;
    for e in WalkDir::new(vault)
            .into_iter()
            .filter_entry(|e| e.depth() == 0 || !e.file_name().to_string_lossy().starts_with('.'))
            .filter_map(|e| e.ok())
    {
        if e.path().extension().map_or(false, |x| x == "md") {
            count += 1;
            if let Ok(m) = e.metadata() {
                if let Ok(t) = m.modified() {
                    if let Ok(d) = t.duration_since(std::time::UNIX_EPOCH) {
                        sum = sum.wrapping_add(d.as_nanos());
                    }
                }
            }
        }
    }
    (count, sum)
}

fn known_vaults(current: &Path) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = vec![current.to_path_buf()];
    if let Some(s) = read_config().get("vaults") {
        for p in s.split(';') {
            let p = p.trim();
            if p.is_empty() {
                continue;
            }
            let pb = PathBuf::from(p);
            if pb.is_dir() && !v.contains(&pb) {
                v.push(pb);
            }
        }
    }
    v
}

fn load_keymap() -> std::collections::HashMap<char, char> {
    let mut map = std::collections::HashMap::new();
    if let Some(s) = read_config().get("remap") {
        for pair in s.split(',') {
            let mut it = pair.trim().chars();
            if let (Some(from), Some(':'), Some(to), None) =
                (it.next(), it.next(), it.next(), it.next())
            {
                map.insert(from, to);
            }
        }
    }
    map
}

fn config_path() -> Option<PathBuf> {
    let base = env::var_os("APPDATA")
        .map(PathBuf::from)
        .or_else(|| env::var_os("XDG_CONFIG_HOME").map(PathBuf::from))
        .or_else(|| env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))?;
    Some(base.join("shale").join("config.txt"))
}

fn read_config() -> std::collections::HashMap<String, String> {
    let mut map = std::collections::HashMap::new();
    if let Some(text) = config_path().and_then(|p| fs::read_to_string(p).ok()) {
        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if let Some((k, v)) = line.split_once('=') {
                map.insert(
                    k.trim().to_lowercase(),
                    v.trim().trim_matches('"').to_string(),
                );
            }
        }
    }
    map
}

fn write_config(map: &std::collections::HashMap<String, String>) -> io::Result<PathBuf> {
    let path = config_path()
        .ok_or_else(|| io::Error::new(io::ErrorKind::Other, "no config folder found"))?;
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    let mut keys: Vec<&String> = map.keys().collect();
    keys.sort();
    let text: String = keys.iter().map(|k| format!("{k} = {}\n", map[*k])).collect();
    fs::write(&path, text)?;
    Ok(path)
} 

fn ui(frame: &mut Frame, app: &mut App) {
    let [body, status_area] =
        Layout::vertical([Constraint::Min(3), Constraint::Length(1)]).areas(frame.area());
    let left_pct: u16 = if app.show_left { 30 } else { 0 };
    let links_h: u16 = if app.show_links { 10 } else { 0 };
    let [left, right] = Layout::horizontal([
        Constraint::Percentage(left_pct),
        Constraint::Percentage(100 - left_pct),
    ])
    .areas(body);
    let [preview_area, links_area] =
        Layout::vertical([Constraint::Min(5), Constraint::Length(links_h)]).areas(right);
    let [out_area, back_area] = 
        Layout::horizontal([Constraint::Percentage(50), Constraint::Percentage(50)])
            .areas(links_area);
    app.notes_area = left;
    app.links_rect = out_area;
    app.back_rect = back_area;
    
    let status_line = if app.confirm_delete {
        let name = app
            .selected_note()
            .map(|n| n.name.clone())
            .unwrap_or_default();
        Line::styled(
            format!(" Move '{name}' to .trash? (y = yes, any other key = cancel) "),
            Style::new().fg(Color::Black).bg(Color::Red),
        )
    } else if !app.status.is_empty() {
        Line::styled(
            format!(" {} ", app.status),
            Style::new().fg(Color::Black).bg(Color::Green),
        )
    } else {
        let vault_name = app
            .vault
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "vault".to_string());
        let words = app
            .selected_note()
            .map_or(0, |n| n.content.split_whitespace().count());
        let pane = match app.focus {
            Focus::Notes => "Notes",
            Focus::Links => "Links",
            Focus::Preview =>  "Preview",
        };
        Line::styled(
            format!(
                " {} | {} pane | {} notes | {} words ? help",
                vault_name,
                pane,
                app.notes.len(),
                words
            ),
            Style::new().fg(Color::DarkGray),
        )
    };
    frame.render_widget(Paragraph::new(status_line), status_area);

    // Left pane: note list
    let items: Vec<ListItem> = app
        .notes
        .iter()
        .map(|n| ListItem::new(n.rel.as_str()))
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

    if app.editor.is_some() {
        let name = app
            .selected_note()
            .map(|n| n.name.clone())
            .unwrap_or_default();
        let title = if app.discard_warned {
            " Unsaved changes! Esc = discard, Ctrl+S = save".to_string()
        } else if app.editor_dirty {
            format!(" Editing {name} [modified] - Ctrl+S save, Esc exit")
        } else {
            format!(" Editing {name} - Ctrl+S save, Esc exit")
        };
        if let Some(ta) = app.editor.as_mut() {
            ta.set_block(
                Block::bordered()
                    .title(title)
                    .border_style(Style::new().fg(Color::Yellow)),
            );
            frame.render_widget(&*ta, preview_area);
        }
    } else {
        let (title, content) = match app.selected_note() {
            Some(n) => (format!(" {} ", n.name), n.content.clone()),
            None => (" Preview ".to_string(), "No notes found.".to_string()),
        };
        let mut rendered =render_markdown(&content, preview_area.width.saturating_sub(2) as usize);
        let max_scroll = rendered.len().saturating_sub(1) as u16;
        app.preview_scroll = app.preview_scroll.min(max_scroll);
        app.cursor_row = app.cursor_row.min(rendered.len().saturating_sub(1));
        let visible_rows = preview_area.height.saturating_sub(2) as usize;
        if app.cursor_row >= app.preview_scroll as usize + visible_rows {
            app.preview_scroll = (app.cursor_row + 1 - visible_rows) as u16;
        } else if app.cursor_row < app.preview_scroll as usize {
            app.preview_scroll = app.cursor_row as u16;
        }
        if let Some(line) = rendered.get_mut(app.cursor_row) {
            for span in &mut line.spans {
                span.style = span.style.add_modifier(Modifier::REVERSED);
            }
        }
        let preview = Paragraph::new(rendered)
            .block(
                Block::bordered().title(title).border_style(
                    if app.focus == Focus::Preview {
                        Style::new().fg(Color::Yellow)
                    } else {
                        Style::new()
                    },
                ),
            )
            .wrap(Wrap { trim: false })
            .scroll((app.preview_scroll, 0));
        frame.render_widget(preview, preview_area);
    }

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

    if app.switcher_open {
        let area = frame.area();
        let w = area.width * 6 / 10;
        let h = area.height.saturating_sub(2).min(14);
        let popup = Rect::new(area.x + (area.width - w) / 2, area.y + 2, w, h);
        frame.render_widget(Clear, popup);

        let [input_area, results_area] =
            Layout::vertical([Constraint::Length(3), Constraint::Min(1)]).areas(popup);
        
        frame.render_widget(
            Paragraph::new(format!("{}_", app.query))
                .block(Block::bordered().title(" Go to note ")),
            input_area,
        );

        let items: Vec<ListItem> = app
            .matches
            .iter()
            .map(|&i| ListItem::new(app.notes[i].name.as_str()))
            .collect();
        let results = List::new(items)
            .block(Block::bordered())
            .highlight_style(Style::new().add_modifier(Modifier::REVERSED))
            .highlight_symbol("> ");
        frame.render_stateful_widget(results, results_area, &mut app.switcher_state);
    }

    if app.prompt_open {
        let area = frame.area();
        let w = area.width * 6 / 10;
        let popup = Rect::new(area.x + (area.width - w) / 2, area.y + 2, w, 3);
        frame.render_widget(Clear, popup);
        frame.render_widget(
            Paragraph::new(format!("{}_", app.prompt_text))
                .block(Block::bordered().title(" New note name (Enter = create, Esc = cancel ")),
            popup,
        );
    }
    
    if app.search_open {
        let area = frame.area();
        let w = area.width * 8 / 10;
        let h = area.height.saturating_sub(4);
        let popup = Rect::new(area.x + (area.width - w) / 2, area.y + 2, w, h);
        frame.render_widget(Clear, popup);

        let [input_area, results_area] =
            Layout::vertical([Constraint::Length(3), Constraint::Min(1)]).areas(popup);
        
        frame.render_widget(
            Paragraph::new(format!("{}_", app.search_query))
                .block(Block::bordered().title(" Search all notes ")),
            input_area,
        );

        let items: Vec<ListItem> = app
            .search_results
            .iter()
            .map(|(i, _, snippet)| {
                ListItem::new(Line::from(vec![
                    Span::styled(
                        format!("{}: ", app.notes[*i].name),
                        Style::new().fg(Color::Cyan),
                    ),
                    Span::raw(snippet.clone()),
                ]))
            })
            .collect();
        let title = format!(" {} matches ", app.search_results.len());
        let results = List::new(items)
            .block(Block::bordered().title(title))
            .highlight_style(Style::new().add_modifier(Modifier::REVERSED))
            .highlight_symbol("> ");
        frame.render_stateful_widget(results, results_area, &mut app.search_state);
    }

    if app.tag_open {
        let area = frame.area();
        let w = area.width * 6 / 10;
        let h = area.height.saturating_sub(4).min(20);
        let popup = Rect::new(area.x + (area.width - w) / 2, area.y + 2, w, h);
        frame.render_widget(Clear, popup);

        let (title, items): (String, Vec<ListItem>) = match app.tag_stage {
            None => (
                format!(" Tags ({}) - Enter = view notes, Esc = close ", app.tag_list.len()),
                app.tag_list
                    .iter()
                    .map(|(t, v)| ListItem::new(format!("#{t} ({})", v.len())))
                    .collect(),
            ),
            Some(ti) => {
                let (t, v) = &app.tag_list[ti];
                (
                    format!(" #{t} - Enter = open, Esc = back "),
                    v.iter()
                        .map(|&i| ListItem::new(app.notes[i].name.clone()))
                        .collect(),
                )
            }
        };
        let list = List::new(items)
            .block(Block::bordered().title(title))
            .highlight_style(Style::new().add_modifier(Modifier::REVERSED))
            .highlight_symbol("> ");
        frame.render_stateful_widget(list, popup, &mut app.tag_state);
    }

    if app.graph_open {
        let area = frame.area();
        let visible = app.graph_visible();
        let sel = app.graph_sel.min(visible.len().saturating_sub(1));

        // Canvas units are braille dots: 2 per cell wide, 4 per cell tall
        let wd = area.width.saturating_sub(2) as f64 * 2.0;
        let hd = area.height.saturating_sub(2) as f64 * 4.0;
        let rx = wd / 2.0 * 0.72;
        let ry = hd / 2.0 * 0.80;
        let r1 = if app.graph_depth >= 2 { 0.5 } else { 0.85 };
        let pos = graph_layout(&visible, rx, ry, r1);

        let mut edges: Vec<(f64, f64, f64, f64)> = Vec::new();
        for (k, &(note_idx, ring)) in visible.iter().enumerate() {
            match ring {
                1 => edges.push((pos[0].0, pos[0].1, pos[k].0, pos[k].1)),
                2 => {
                    let parent = visible
                        .iter()
                        .position(|&(p, r)| r == 1 && app.graph_adj[p].contains(&note_idx));
                    if let Some(p) = parent {
                        edges.push((pos[p].0, pos[p].1, pos[k].0, pos[k].1));
                    }
                }
                _ => {}
            }
        }

        let labels: Vec<(f64, f64, String, Style)> = visible
            .iter()
            .enumerate()
            .map(|(k, &(note_idx, ring))| {
                let name: String = app.notes[note_idx].name.chars().take(18).collect();
                let text = format!("● {name}");
                let mut style = match ring {
                    0 => Style::new().fg(Color::Cyan).add_modifier(Modifier::BOLD),
                    1 => Style::new().fg(Color::White),
                    _ => Style::new().fg(Color::Gray),
                };
                if k == sel {
                    style = Style::new()
                        .fg(Color::Black)
                        .bg(Color::Yellow)
                        .add_modifier(Modifier::BOLD);
                }
                // Center the label on the node
                let x = pos[k].0 - text.chars().count() as f64;
                (x, pos[k].1, text, style)
            })
            .collect();

        let title = format!(
            " Graph: {} ({} shown) - j/k move, Enter recenter, 1/2 depth, Esc close ",
            app.notes[app.graph_center].name,
            visible.len()
        );
        let canvas = Canvas::default()
            .block(Block::bordered().title(title))
            .marker(Marker::Braille)
            .x_bounds([-wd / 2.0, wd / 2.0])
            .y_bounds([-hd / 2.0, hd / 2.0])
            .paint(|ctx| {
                for &(x1, y1, x2, y2) in &edges {
                    ctx.draw(&CanvasLine {
                        x1,
                        y1,
                        x2,
                        y2,
                        color: Color::DarkGray,
                    });
                }
                for (x, y, text, style) in &labels {
                    ctx.print(*x, *y, Line::styled(text.clone(), *style));
                }
            });
        frame.render_widget(Clear, area);
        frame.render_widget(canvas, area);
    }

    if app.help_open {
        let head = Style::new().fg(Color::Cyan).add_modifier(Modifier::BOLD);
        let lines: Vec<Line> = vec![
            Line::styled("Move around", head),
            Line::from("  j / k, arrows   move in the focused list"),
            Line::from("  Tab             switch Notes / Links focus"),
            Line::from("  Enter           follow the selected link"),
            Line::from("  Backspace, h    back         l  forward"),
            Line::from("  PgUp / PgDn     scroll the preview (or mouse wheel)"),
            Line::from("  click           select a note or follow a link"),
            Line::from("  Ctrl+D/Ctrl+U   half-page down/up"),
            Line::from("  Ctrl+B/Ctrl+O   hide or show notes list / links panes"),
            Line::styled("Find", head),
            Line::from("  Ctrl+P          quick switcher"),
            Line::from("  /               search inside all notes"),
            Line::from("  t               browse tags"),
            Line::from("  g               graph view"),
            Line::from("  u               unresolved links and orphans"),
            Line::from("  O               outline (headings of this note)"),
            Line::styled("Write", head),
            Line::from("  e               edit in the app (Ctrl+S save, Esc exit)"),
            Line::from("  E               edit in an external editor"),
            Line::from("  n               new note        d   daily note"),
            Line::from("  R               rename (updates links)    D   delete to .trash"),
            Line::from("  T               task list (Space toggles)"),
            Line::from("  o               recent notes"),
            Line::styled("Other", head),
            Line::from("  r               reload the vault from disk"),
            Line::from("  ?               this help"),
            Line::from("  q, Esc          quit"),
            Line::from("  s               toggle sort (name / modified)"),
            Line::from("  Ctrl+g          switch vault"),
            Line::from(""),
            Line::styled(
                "j/k or PgUp/PgDn scroll, any other key closes", 
                Style::new().fg(Color::DarkGray)
            ),
        ];
        let area = frame.area();
        let w = (area.width * 7 / 10).max(50).min(area.width);
        let h = (lines.len() as u16 + 2).min(area.height);
        let popup = Rect::new(
            area.x + (area.width - w) / 2,
            area.y + area.height.saturating_sub(h) / 2,
            w,
            h,
        );
        frame.render_widget(Clear, popup);
        let max_scroll = (lines.len() as u16).saturating_sub(h.saturating_sub(2));
        app.help_scroll = app.help_scroll.min(max_scroll);
        frame.render_widget(
            Paragraph::new(lines)
                .block(Block::bordered().title(" Help "))
                .scroll((app.help_scroll, 0)),
            popup,
        );
    }

    if app.tasks_open {
        let area = frame.area();
        let w = area.width * 8 / 10;
        let h = area.height.saturating_sub(4);
        let popup = Rect::new(area.x + (area.width - w) / 2, area.y + 2, w, h);
        frame.render_widget(Clear, popup);
        let items: Vec<ListItem> = app
            .tasks
            .iter()
            .map(|t| {
                let (mark, style) = if t.done {
                    ("☑ ", Style::new().fg(Color::Green))
                } else {
                    ("☐ ", Style::new().fg(Color::Yellow))
                };
                ListItem::new(Line::from(vec![
                    Span::styled(mark, style),
                    Span::raw(t.text.clone()),
                    Span::styled(
                        format!("  - {}", app.notes[t.note].name),
                        Style::new().fg(Color::DarkGray),
                    ),
                ]))
            })
            .collect();
        let title = format!(
            " Tasks ({}) - Space = toggle, Enter = open note, a = {} done, Esc = close",
            app.tasks.len(),
            if app.tasks_show_done { "hide" } else { "show" } 
        );
        let list = List::new(items)
            .block(Block::bordered().title(title))
            .highlight_style(Style::new().add_modifier(Modifier::REVERSED))
            .highlight_symbol("> ");
        frame.render_stateful_widget(list, popup, &mut app.task_state);
    }

    if app.recent_open {
        let area = frame.area();
        let w = area.width * 5 / 10;
        let h = (app.recent_items.len() as u16 + 2)
            .max(3)
            .min(area.height.saturating_sub(4));
        let popup = Rect::new(area.x + (area.width - w) / 2, area.y + 2, w, h,);
        frame.render_widget(Clear, popup);
        let items: Vec<ListItem> = app
            .recent_items
            .iter()
            .map(|&i| ListItem::new(app.notes[i].name.as_str()))
            .collect();
        let list = List::new(items)
            .block(Block::bordered().title(" Recent - Enter = open, Esc = close "))
            .highlight_style(Style::new().add_modifier(Modifier::REVERSED))
            .highlight_symbol("> ");
        frame.render_stateful_widget(list, popup, &mut app.recent_state);
    }

    if app.trash_open {
        let area = frame.area();
        let w = area.width * 6 /10;
        let h = (app.trash_items.len() as u16 + 2)
            .max(3)
            .min(area.height.saturating_sub(4));
        let popup = Rect::new(area.x + (area.width - w) / 2, area.y + 2, w, h);
        frame.render_widget(Clear, popup);
        let items: Vec<ListItem> = app
            .trash_items
            .iter()
            .map(|(_, name)| ListItem::new(name.clone()))
            .collect();
        let list = List::new(items)
            .block(Block::bordered().title(" Trash - Enter = restore, Esc = close "))
            .highlight_style(Style::new().add_modifier(Modifier::REVERSED))
            .highlight_symbol("> ");
        frame.render_stateful_widget(list, popup, &mut app.trash_state);
    }

    if app.health_open {
        let area = frame.area();
        let w = area.width * 8 / 10;
        let h = area.height.saturating_sub(4);
        let popup = Rect::new(area.x + (area.width - w) / 2, area.y + 2, w, h);
        frame.render_widget(Clear, popup);
        let items: Vec<ListItem> = app
            .health_items
            .iter()
            .map(|(_, text)| {
                let color = if text.starts_with("missing") || text.starts_with("unlinked") {
                    Color::Yellow
                } else {
                    Color::DarkGray
                };
                ListItem::new(Span::styled(text.clone(), Style::new().fg(color)))
            })
            .collect();
        let title = format!(
            " Unresolved links, orphans and unlinked mentions ({}) - Enter = open, Esc = close ",
            app.health_items.len()
        );
        let list = List::new(items)
            .block(Block::bordered().title(title))
            .highlight_style(Style::new().add_modifier(Modifier::REVERSED))
            .highlight_symbol("> ");
        frame.render_stateful_widget(list, popup, &mut app.health_state);
    }

    if app.outline_open {
        let area = frame.area();
        let w = area.width * 6 / 10;
        let h = area.height.saturating_sub(4).min(24);
        let popup = Rect::new(area.x + (area.width - w) / 2, area.y + 2, w, h);
        frame.render_widget(Clear, popup);
        let items: Vec<ListItem> = app
            .outline_items
            .iter()
            .map(|(level, _, text)| {
                let color = match level {
                    1 => Color::Cyan,
                    2 => Color::Blue,
                    _ => Color::Magenta,
                };
                ListItem::new(Span::styled(
                    format!("{}{}", " ".repeat(level - 1), text),
                    Style::new().fg(color),
                ))
            })
            .collect();
        let title = format!(
            " Outline ({}) - Enter = jump, Esc = close ",
            app.outline_items.len()
        );
        let list = List::new(items)
            .block(Block::bordered().title(title))
            .highlight_style(Style::new().add_modifier(Modifier::REVERSED))
            .highlight_symbol("> ");
        frame.render_stateful_widget(list, popup, &mut app.outline_state);
    }

    if app.vault_open {
        let area = frame.area();
        let w = area.width * 7 / 10;
        let h = (app.vault_items.len() as u16 + 2)
            .max(3)
            .min(area.height.saturating_sub(4));
        let popup = Rect::new(area.x + (area.width - w) / 2, area.y + 2, w, h);
        frame.render_widget(Clear, popup);
        let items: Vec<ListItem> = app
            .vault_items
            .iter()
            .map(|p| {
                let mark = if *p == app.vault { "◆" } else { " " };
                ListItem::new(format!("{mark} {}", p.display()))
            })
            .collect();
        let list = List::new(items)
            .block(Block::bordered().title(" Vaults - Enter = open, Esc = close "))
            .highlight_style(Style::new().add_modifier(Modifier::REVERSED))
            .highlight_symbol("> ");
        frame.render_stateful_widget(list, popup, &mut app.vault_state);
    }

    if app.ac_open {
        let n = app.ac_matches.len() as u16;
        let h = (n + 2).min(preview_area.height);
        let w = preview_area.width.min(36);
        let popup = Rect::new(
            preview_area.x + preview_area.width.min(2),
            preview_area.y + preview_area.height.saturating_sub(h + 1),
            w,
            h,
        );
        frame.render_widget(Clear, popup);
        let items: Vec<ListItem> = app
            .ac_matches
            .iter()
            .map(|&i| ListItem::new(app.notes[i].name.as_str()))
            .collect();
        let list = List::new(items)
            .block(Block::bordered().title(" Link to... "))
            .highlight_style(Style::new().add_modifier(Modifier::REVERSED))
            .highlight_symbol("> ");
        frame.render_stateful_widget(list, popup, &mut app.ac_state);
    }
}

fn open_editor(terminal: &mut DefaultTerminal, app: &mut App) -> io::Result<()> {
    let Some(path) = app.selected_note().map(|n| n.path.clone()) else {
        return Ok (());
    };
    let editor = env::var("EDITOR")
    .ok()
    .or_else(|| read_config().get("editor").cloned())
    .unwrap_or_else(|| {
        if cfg!(windows) { "notepad".to_string() } else { "vi".to_string() }
    });

    // Hand the terminal back before launching the editor
    execute!(io::stdout(), DisableMouseCapture)?;
    ratatui::restore();

    let _ = if cfg!(windows) {
        Command::new("cmd").arg("/C").arg(&editor).arg(&path).status()
    } else {
        let mut parts = editor.split_whitespace();
        let prog = parts.next().unwrap_or("vi");
        Command::new(prog).args(parts).arg(&path).status()
    };

    // Take the terminal back
    *terminal = ratatui::init();
    execute!(io::stdout(), EnableMouseCapture)?;
    app.refresh_after_edit();
    Ok(())
}

fn run(terminal: &mut DefaultTerminal, app: &mut App) -> io::Result<()> {
    while !app.should_quit {
        terminal.draw(|f| ui(f, app))?;
        if event::poll(Duration::from_millis(500))? {
            match event::read()? {
                Event::Key(key) if key.kind == KeyEventKind::Press => {
                    app.status.clear();
                    if app.editor.is_some() {
                        app.handle_editor_key(key);
                    } else if key.modifiers.contains(KeyModifiers::CONTROL)
                        && key.code == KeyCode::Char('p')
                    {
                        app.open_switcher();
                    } else if key.modifiers.contains(KeyModifiers::CONTROL)
                        && matches!(key.code, KeyCode::Char('d') | KeyCode::Char('u'))
                    {
                        if !app.any_popup() {
                            let rows = terminal.size()?.height;
                            let half = (rows.saturating_sub(11) / 2).max(1) as i32;
                            let delta = if key.code == KeyCode::Char('d') { half } else { -half };
                            app.scroll_preview(delta);
                        }
                    } else if key.modifiers.contains(KeyModifiers::CONTROL)
                        && matches!(key.code, KeyCode::Char('b') | KeyCode::Char('o'))
                    {
                        if !app.any_popup() {
                            if key.code == KeyCode::Char('b') {
                                app.show_left = !app.show_left;
                            } else {
                                app.show_links = !app.show_links;
                            }
                        }
                    } else if key.modifiers.contains(KeyModifiers::CONTROL)
                        && key.code == KeyCode::Char('g')
                    {
                        if !app.any_popup() {
                            app.open_vaults();
                        }
                    } else {
                        app.handle_key(key.code);
                    }
                }
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
        app.tick();
        if app.edit_requested {
            app.edit_requested = false;
            open_editor(terminal, app)?;
        }
    }
    Ok(())
}

fn main() -> io::Result<()> {
    let args: Vec<String> = env::args().skip(1).collect();
    let mut config = read_config();

    if args.len() == 2 && args[0] == "--set-vault" {
        config.insert("vault".to_string(), args[1].clone());
        let path = write_config(&config)?;
        println!("Saved vault path to {}", path.display());
        return Ok(());
    }

    if args.len() == 2 && args[0] == "--add-vault" {
        let existing = config.get("vaults").cloned().unwrap_or_default();
        let mut list: Vec<String> = existing
            .split(';')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        if !list.contains(&args[1]) {
            list.push(args[1].clone());
        }
        config.insert("vaults".to_string(), list.join(";"));
        let path = write_config(&config)?;
        println!("Saved vault list to {}", path.display());
        return Ok(());
    }

    let vault = args
        .first()
        .cloned()
        .or_else(|| config.get("vault").cloned())
        .unwrap_or_else(|| ".".to_string());
    if !Path::new(&vault).is_dir() {
        eprintln!("Vault folder not found: {vault}");
        return Ok(());
    }

    let mut app = App::new(Path::new(&vault));

    let mut terminal = ratatui::init();
    execute!(io::stdout(), EnableMouseCapture)?;
    let result = run(&mut terminal, &mut app);
    execute!(io::stdout(), DisableMouseCapture)?;
    ratatui::restore();
    result
}