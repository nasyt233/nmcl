mod app;
mod ui;
mod worker;

use anyhow::Result;
use app::{App, EditField, Tab, WorkerMsg};
use crossterm::{
    event::{
        self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEvent, KeyEventKind,
        KeyModifiers,
    },
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::{
    io,
    path::PathBuf,
    sync::mpsc::{self, Receiver, Sender},
    time::Duration,
};

fn main() -> Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let mut terminal = Terminal::new(CrosstermBackend::new(stdout))?;

    let mc_dir = worker::meta::default_minecraft_dir().unwrap_or_else(|| PathBuf::from(".minecraft"));
    let mut app = App::new(mc_dir);
    app.push_log(format!("MC 目录: {}", app.mc_dir.display()));

    let versions = worker::meta::scan_versions(&app.mc_dir);
    let n = versions.len();
    app.versions = versions;
    if n == 0 {
        app.push_log("未找到本地版本，去「下载」页下载");
    } else {
        app.push_log(format!("已加载 {n} 个本地版本"));
        app.version_selected = Some(0);
    }

    let (tx, rx) = mpsc::channel::<WorkerMsg>();
    worker::download::start_fetch_manifest(tx.clone(), app.cancel.clone());

    let res = run(&mut terminal, &mut app, &rx, &tx);
    app.flush_log();

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen, DisableMouseCapture)?;
    terminal.show_cursor()?;
    res
}

fn run<B: ratatui::backend::Backend>(
    terminal: &mut Terminal<B>,
    app: &mut App,
    rx: &Receiver<WorkerMsg>,
    tx: &Sender<WorkerMsg>,
) -> Result<()> {
    loop {
        while let Ok(msg) = rx.try_recv() {
            app.handle_worker_msg(msg);
        }

        if app.needs_hot && !app.resource_searching && !app.busy {
            app.needs_hot = false;
            app.submit_hot_search(tx.clone());
        }

        terminal.draw(|f| ui::draw(f, app))?;

        if event::poll(Duration::from_millis(50))? {
            match event::read()? {
                Event::Key(key) if key.kind == KeyEventKind::Press => {
                    if handle_key(app, key, tx) {
                        break;
                    }
                }
                Event::Mouse(m) => app.handle_mouse(m, tx.clone()),
                _ => {}
            }
        }

        if app.should_quit {
            break;
        }
    }
    Ok(())
}

fn handle_key(app: &mut App, key: KeyEvent, tx: &Sender<WorkerMsg>) -> bool {
    // 弹窗层优先
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
        KeyCode::Char('q') => return true,
        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => return true,
        KeyCode::Char('x') => app.cancel_current(),
        KeyCode::PageUp | KeyCode::Char('[') => { app.prev_page(tx.clone()); return false; }
        KeyCode::PageDown | KeyCode::Char(']') => { app.next_page(tx.clone()); return false; }
        KeyCode::Char('1') => app.tab = Tab::Home,
        KeyCode::Char('2') => app.tab = Tab::Versions,
        KeyCode::Char('3') => app.tab = Tab::Downloads,
        KeyCode::Char('4') => app.tab = Tab::Settings,
        KeyCode::Tab => app.tab = app.tab.next(),
        KeyCode::BackTab => app.tab = app.tab.prev(),
        KeyCode::Up | KeyCode::Char('k') => app.handle_scroll_up(),
        KeyCode::Down | KeyCode::Char('j') => app.handle_scroll_down(),
        KeyCode::Enter => app.confirm(tx.clone()),
        _ => {}
    }
    false
}