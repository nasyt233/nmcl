use crate::app::{App, ClickTarget, UiLayout};
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
    Frame,
};
use super::common::*;

pub fn draw_home(f: &mut Frame, app: &mut App, area: Rect, layout: &mut UiLayout) {
    let cols = Layout::default().direction(Direction::Horizontal).constraints([Constraint::Percentage(30), Constraint::Percentage(70)]).split(area);
    let left = Layout::default().direction(Direction::Vertical).constraints([Constraint::Length(6), Constraint::Min(5), Constraint::Length(3)]).split(cols[0]);

    // 1. 角色卡片
    let profile = &app.profiles[app.current_profile];
    let p_block = Block::default().borders(Borders::ALL).border_style(Style::default().fg(GOLD)).style(Style::default().bg(BG));
    f.render_widget(p_block.clone(), left[0]);
    f.render_widget(Paragraph::new(vec![
        Line::from(Span::styled(format!(" 角色 : {}", profile.name), Style::default().fg(TEXT).add_modifier(Modifier::BOLD))),
        Line::from(Span::styled(" 离线模式", Style::default().fg(GRAY))),
    ]).style(Style::default().bg(BG)), p_block.inner(left[0]));
    clickable(layout, left[0], ClickTarget::OpenProfileManager);

    // 2. 运行中列表
    let run_block = Block::default().title(" 运行中 ").borders(Borders::ALL).border_style(Style::default().fg(GRAY_DIM)).style(Style::default().bg(BG));
    let inner = run_block.inner(left[1]);
    f.render_widget(run_block, left[1]);

    for (i, game) in app.running_games.iter().enumerate() {
        if i as u16 >= inner.height { break; }
        let y = inner.y + i as u16;
        let row = Rect::new(inner.x, y, inner.width, 1);
        
        let text = format!(" ▶ {} ({})", game.version, game.player);
        f.render_widget(Paragraph::new(text).style(Style::default().fg(GRASS)), row);
        
        let close_rect = Rect::new(inner.x + inner.width.saturating_sub(6), y, 6, 1);
        f.render_widget(Paragraph::new(" [关闭] ").style(Style::default().fg(RED)), close_rect);
        clickable(layout, close_rect, ClickTarget::KillGame(i));
    }

    // 3. 版本选择按钮
    f.render_widget(Paragraph::new(" 版本选择 ").alignment(Alignment::Center)
        .block(Block::default().borders(Borders::ALL).border_style(Style::default().fg(GRAY_DIM))).style(Style::default().bg(BG)), left[2]);
    clickable(layout, left[2], ClickTarget::OpenHomeVersions);

    let right = Layout::default().direction(Direction::Vertical).constraints([Constraint::Fill(1), Constraint::Length(3), Constraint::Length(1), Constraint::Length(3), Constraint::Fill(1)]).split(cols[1]);
    let current_ver = app.selected_local().map(|v| v.name.as_str()).unwrap_or("未选择版本");

    let v_cols = Layout::default().direction(Direction::Horizontal)
        .constraints([Constraint::Fill(1), Constraint::Length(3), Constraint::Length(24), Constraint::Length(3), Constraint::Fill(1)]).split(right[1]);
    f.render_widget(Paragraph::new(" ◀ ").alignment(Alignment::Center).style(Style::default().fg(GRAY)), v_cols[1]);
    clickable(layout, v_cols[1], ClickTarget::PrevVersion);
    f.render_widget(Paragraph::new(Line::from(Span::styled(current_ver, Style::default().fg(TEXT).add_modifier(Modifier::BOLD)))).alignment(Alignment::Center), v_cols[2]);
    f.render_widget(Paragraph::new(" ▶ ").alignment(Alignment::Center).style(Style::default().fg(GRAY)), v_cols[3]);
    clickable(layout, v_cols[3], ClickTarget::NextVersion);

    let can_launch = app.selected_local().is_some();
    let btn_style = if can_launch { Style::default().bg(GRASS).fg(Color::Black).add_modifier(Modifier::BOLD) } else { Style::default().bg(GRAY_DIM).fg(GRAY) };
    let l_rect = Layout::default().direction(Direction::Horizontal).constraints([Constraint::Fill(1), Constraint::Length(24), Constraint::Fill(1)]).split(right[3])[1];
    
    f.render_widget(Paragraph::new(" 启动游戏 ").alignment(Alignment::Center)
        .block(Block::default().borders(Borders::ALL).border_style(Style::default().fg(GRASS))).style(btn_style), l_rect);
    if can_launch { clickable(layout, l_rect, ClickTarget::QuickLaunch); }
}

pub fn draw_home_version_modal(f: &mut Frame, app: &mut App) {
    let area = centered(f.area(), 40, 15);
    f.render_widget(Clear, area);
    let block = Block::default().title(" 选择版本 ").borders(Borders::ALL).border_style(Style::default().fg(GOLD)).style(Style::default().bg(BG));
    let inner = block.inner(area);
    f.render_widget(block, area);

    if app.versions.is_empty() {
        f.render_widget(Paragraph::new(" 暂无版本").alignment(Alignment::Center).style(Style::default().fg(GRAY)), inner);
        return;
    }

    let h = inner.height as usize;
    let offset = if app.home_version_selected >= h { app.home_version_selected - h + 1 } else { 0 };
    
    for i in offset..(offset + h).min(app.versions.len()) {
        let v = &app.versions[i];
        let y = inner.y + (i - offset) as u16;
        let is_sel = i == app.home_version_selected;
        let style = if is_sel { Style::default().bg(GOLD).fg(Color::Black) } else { Style::default().fg(TEXT) };
        let row = Rect::new(inner.x, y, inner.width, 1);
        
        f.render_widget(Paragraph::new(format!(" {}", v.name)).style(style), row);
        clickable(&mut app.layout, row, ClickTarget::HomeVersionSelect(i));
    }
}