use crate::app::{App, ClickTarget};
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
    Frame,
};
use super::common::*;

pub fn draw_profile_modal(f: &mut Frame, app: &mut App) {
    let area = centered(f.area(), 50, 16);
    f.render_widget(Clear, area);

    let block = Block::default()
        .title(Line::from(Span::styled(" 角色管理 ", Style::default().fg(GOLD).add_modifier(Modifier::BOLD))))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(GOLD))
        .style(Style::default().bg(BG));
    let inner = block.inner(area);
    f.render_widget(block, area);

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(5), Constraint::Length(3)])
        .split(inner);

    let list_inner = Block::default().borders(Borders::ALL).inner(rows[0]);
    f.render_widget(Block::default().borders(Borders::ALL).border_style(Style::default().fg(GRAY_DIM)), rows[0]);
    
    let h = list_inner.height as usize;
    let start = 0;
    for i in start..(start+h).min(app.profiles.len()) {
        let y = list_inner.y + (i-start) as u16;
        let row = Rect::new(list_inner.x, y, list_inner.width, 1);
        let p = &app.profiles[i];
        let is_sel = Some(i) == app.profile_selected;
        let prefix = if i == app.current_profile { "● " } else { "  " };
        let style = if is_sel { Style::default().bg(GOLD).fg(Color::Black).add_modifier(Modifier::BOLD) } else { Style::default().fg(TEXT).bg(BG) };
        f.render_widget(Paragraph::new(Line::from(Span::styled(format!(" {}{}", prefix, p.name), style))).style(style), row);
        clickable(&mut app.layout, row, ClickTarget::ProfileSelect(i));
    }

    let btn_cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(12), Constraint::Length(12), Constraint::Fill(1), Constraint::Length(12)])
        .split(rows[1]);
        
    let new_rect = Rect::new(btn_cols[0].x+1, btn_cols[0].y+1, 10, 1);
    f.render_widget(Paragraph::new(" 新建 ").alignment(Alignment::Center).style(Style::default().bg(GRASS).fg(Color::Black)), new_rect);
    clickable(&mut app.layout, new_rect, ClickTarget::ProfileCreate);

    let del_rect = Rect::new(btn_cols[1].x+1, btn_cols[1].y+1, 10, 1);
    f.render_widget(Paragraph::new(" 删除 ").alignment(Alignment::Center).style(Style::default().bg(RED).fg(Color::Black)), del_rect);
    if let Some(i) = app.profile_selected { if i > 0 { clickable(&mut app.layout, del_rect, ClickTarget::ProfileDelete(i)); } }

    let ok_rect = Rect::new(btn_cols[3].x+1, btn_cols[3].y+1, 10, 1);
    f.render_widget(Paragraph::new(" 确认 ").alignment(Alignment::Center).style(Style::default().bg(GOLD).fg(Color::Black)), ok_rect);
    clickable(&mut app.layout, ok_rect, ClickTarget::CloseModal);

    if inner.height > 12 {
        f.render_widget(Paragraph::new(Line::from(Span::styled(" 提示: n 新建 · d 删除 · Enter 确认", Style::default().fg(GRAY)))), Rect::new(inner.x, inner.y+inner.height-1, inner.width, 1));
    }
}