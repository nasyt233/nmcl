use crate::app::{App, ClickTarget, UiLayout};
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};
use super::common::*;

pub fn draw_home(f: &mut Frame, app: &mut App, area: Rect, layout: &mut UiLayout) {
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(30), Constraint::Percentage(70)])
        .split(area);

    // ================== 左侧面板 ==================
    let left_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(6),  // 角色卡片
            Constraint::Fill(1),    // 中间留白
            Constraint::Length(3),  // 版本选择按钮
        ])
        .split(cols[0]);

    // 角色卡片
    let profile = &app.profiles[app.current_profile];
    let profile_block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(GOLD))
        .style(Style::default().bg(BG));
    let profile_inner = profile_block.inner(left_chunks[0]);
    f.render_widget(profile_block, left_chunks[0]);
    
    f.render_widget(Paragraph::new(vec![
        Line::from(Span::styled(format!(" 角色 : {}", profile.name), Style::default().fg(TEXT).add_modifier(Modifier::BOLD))),
        Line::from(Span::styled(" 离线模式", Style::default().fg(GRAY))),
    ]).style(Style::default().bg(BG)), profile_inner);
    clickable(layout, left_chunks[0], ClickTarget::OpenProfileManager);

    // 版本选择按钮
    let _btn_rect = Rect::new(left_chunks[2].x + 1, left_chunks[2].y + 1, left_chunks[2].width.saturating_sub(2), 1);
    f.render_widget(
        Paragraph::new(Line::from(Span::styled(" 版本选择 ", Style::default().fg(TEXT))))
            .alignment(Alignment::Center)
            .block(Block::default().borders(Borders::ALL).border_style(Style::default().fg(GRAY_DIM)))
            .style(Style::default().bg(BG)),
        left_chunks[2]
    );
    clickable(layout, left_chunks[2], ClickTarget::Tab(crate::app::Tab::Versions));

    // ================== 右侧面板 ==================
    let right_block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(GRAY_DIM))
        .style(Style::default().bg(BG));
    let right_inner = right_block.inner(cols[1]);
    f.render_widget(right_block, cols[1]);

    let right_center = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Fill(1),
            Constraint::Length(3), // 版本号
            Constraint::Length(1), // 留白
            Constraint::Length(3), // 启动按钮
            Constraint::Fill(1),
        ])
        .split(right_inner);

    let current_ver = app.selected_local().map(|v| v.name.as_str()).unwrap_or("未选择版本");
    
    // 大号版本字
    f.render_widget(
        Paragraph::new(Line::from(Span::styled(current_ver, Style::default().fg(TEXT).add_modifier(Modifier::BOLD))))
            .alignment(Alignment::Center),
        right_center[1],
    );

    // 大号启动按钮
    let can_launch = app.selected_local().is_some();
    let btn_style = if can_launch {
        Style::default().bg(GRASS).fg(Color::Black).add_modifier(Modifier::BOLD)
    } else {
        Style::default().bg(GRAY_DIM).fg(GRAY)
    };
    
    let launch_rect = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Fill(1), Constraint::Length(24), Constraint::Fill(1)])
        .split(right_center[3])[1];
        
    f.render_widget(
        Paragraph::new(Line::from(Span::styled(" 启动游戏 ", btn_style)))
            .alignment(Alignment::Center)
            .block(Block::default().borders(Borders::ALL).border_style(Style::default().fg(GRASS)))
            .style(btn_style),
        launch_rect,
    );
    if can_launch { clickable(layout, launch_rect, ClickTarget::QuickLaunch); }
}