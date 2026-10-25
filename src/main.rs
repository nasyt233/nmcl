mod app;
mod ui;
mod worker;

use anyhow::Result;
use app::{App, EditField, FocusArea, Tab, WorkerMsg};
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::{io, path::PathBuf, sync::mpsc::{self, Receiver, Sender}, time::Duration};

fn main() -> Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let mut terminal = Terminal::new(CrosstermBackend::new(stdout))?;

    let mc_dir = worker::meta::default_minecraft_dir().unwrap_or_else(|| PathBuf::from(".minecraft"));
    let mut app = App::new(mc_dir);
    let versions = worker::meta::scan_versions(&app.mc_dir);
    app.versions = versions;
    
    if let Some(v_name) = &app.settings.selected_version {
        if let Some(idx) = app.versions.iter().position(|v| v.name == *v_name) {
            app.version_selected = Some(idx);
        }
    }
    if app.version_selected.is_none() && !app.versions.is_empty() { app.version_selected = Some(0); }

    let (tx, rx) = mpsc::channel::<WorkerMsg>();
    worker::download::start_fetch_manifest(tx.clone(), app.cancel.clone());

    let res = run(&mut terminal, &mut app, &rx, &tx);
    app.flush_log();
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen, DisableMouseCapture)?;
    terminal.show_cursor()?;
    res
}

fn run<B: ratatui::backend::Backend>(terminal: &mut Terminal<B>, app: &mut App, rx: &Receiver<WorkerMsg>, tx: &Sender<WorkerMsg>) -> Result<()> {
    loop {
        while let Ok(msg) = rx.try_recv() { app.handle_worker_msg(msg); }
        if app.needs_hot && !app.resource_searching && !app.busy { app.needs_hot = false; app.submit_hot_search(tx.clone()); }
        terminal.draw(|f| ui::draw(f, app))?;

        if event::poll(Duration::from_millis(50))? {
            match event::read()? {
                Event::Key(key) if key.kind == KeyEventKind::Press => { if handle_key(app, key, tx) { break; } }
                Event::Mouse(m) => app.handle_mouse(m, tx.clone()),
                _ => {}
            }
        }
        if app.should_quit { break; }
    }
    Ok(())
}

fn handle_key(app: &mut App, key: KeyEvent, tx: &Sender<WorkerMsg>) -> bool {
    if app.home_version_modal_open {
        match key.code {
            KeyCode::Esc => app.home_version_modal_open = false,
            KeyCode::Up | KeyCode::Char('k') => { if app.home_version_selected > 0 { app.home_version_selected -= 1; } }
            KeyCode::Down | KeyCode::Char('j') => { if app.home_version_selected + 1 < app.versions.len() { app.home_version_selected += 1; } }
            KeyCode::Enter => {
                if !app.versions.is_empty() {
                    app.version_selected = Some(app.home_version_selected);
                    app.settings.selected_version = Some(app.versions[app.home_version_selected].name.clone());
                    app.save_settings();
                }
                app.home_version_modal_open = false;
            }
            _ => {}
        }
        return false;
    }

    if app.profile_modal_open { return app.handle_profile_modal_key(key); }
    if app.download_modal_open { return app.handle_download_modal_key(key, tx.clone()); }
    if app.resource_modal_open { return app.handle_resource_modal_key(key, tx.clone()); }
    if app.modpack_modal_open { return app.handle_modpack_modal_key(key, tx.clone()); }

    if app.search_editing {
        match key.code {
            KeyCode::Enter => { app.search_editing = false; app.submit_resource_search(tx.clone()); }
            KeyCode::Esc => { app.search_editing = false; app.search_buffer.clear(); }
            KeyCode::Backspace => { app.search_buffer.pop(); }
            KeyCode::Char(c) => app.search_buffer.push(c),
            _ => {}
        }
        return false;
    }

    if app.editing != EditField::None {
        match key.code {
            KeyCode::Enter => app.commit_edit(),
            KeyCode::Esc => app.cancel_edit(),
            KeyCode::Backspace => { app.edit_buffer.pop(); }
            KeyCode::Char(c) => app.edit_buffer.push(c),
            _ => {}
        }
        return false;
    }

    match key.code {
        KeyCode::Char('q') if !app.busy => return true,
        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) && !app.busy => return true,
        KeyCode::Char('x') => app.cancel_current(),
        KeyCode::Char('1') => app.tab = Tab::Home,
        KeyCode::Char('2') => app.tab = Tab::Versions,
        KeyCode::Char('3') => app.tab = Tab::Downloads,
        KeyCode::Char('4') => app.tab = Tab::Settings,
        KeyCode::Tab => app.tab = app.tab.next(),
        KeyCode::BackTab => app.tab = app.tab.prev(),
        
        KeyCode::Left => { app.focus_area = FocusArea::Left; return false; }
        KeyCode::Right => { app.focus_area = FocusArea::Right; return false; }

        KeyCode::Up | KeyCode::Char('k') => { app.nav_up(); return false; }
        KeyCode::Down | KeyCode::Char('j') => { app.nav_down(); return false; }

        KeyCode::Char('[') if app.tab == Tab::Downloads => app.prev_page(tx.clone()),
        KeyCode::Char(']') if app.tab == Tab::Downloads => app.next_page(tx.clone()),
        KeyCode::Enter => app.confirm(tx.clone()),
        _ => {}
    }
    false
}