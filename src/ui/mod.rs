pub mod common;
pub mod download_modal;
pub mod home;
pub mod modals;
pub mod nav;
pub mod profiles;
pub mod resources;
pub mod settings;
pub mod versions;

use crate::app::{App, Tab, UiLayout};
use ratatui::{
    layout::{Constraint, Direction, Layout},
    style::Style,
    widgets::Block,
    Frame,
};
use common::BG;

pub fn draw(f: &mut Frame, app: &mut App) {
    app.layout = UiLayout::default();
    let area = f.area();
    f.render_widget(Block::default().style(Style::default().bg(BG)), area);

    let progress_h = if app.busy { 1 } else { 0 };
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(progress_h),
            Constraint::Min(10),
            Constraint::Length(7),
            Constraint::Length(1),
        ])
        .split(area);

    nav::draw_nav(f, app, chunks[0]);
    nav::draw_separator(f, chunks[1]);
    if app.busy { nav::draw_progress(f, app, chunks[2]); }

    let mut layout = std::mem::take(&mut app.layout);
    match app.tab {
        Tab::Home => home::draw_home(f, app, chunks[3], &mut layout),
        Tab::Versions => versions::draw_versions(f, app, chunks[3], &mut layout),
        Tab::Downloads => resources::draw_resources(f, app, chunks[3], &mut layout),
        Tab::Settings => settings::draw_settings(f, app, chunks[3], &mut layout),
    }
    nav::draw_logs(f, app, chunks[4]);
    nav::draw_help(f, chunks[5]);
    app.layout = layout;

    // 弹窗
    if app.home_version_modal_open { home::draw_home_version_modal(f, app); }
    if app.profile_modal_open { profiles::draw_profile_modal(f, app); }
    if app.download_modal_open { download_modal::draw_download_modal(f, app); }
    if app.resource_modal_open { modals::draw_resource_modal(f, app); }
    if app.modpack_modal_open { modals::draw_modpack_modal(f, app); }
}