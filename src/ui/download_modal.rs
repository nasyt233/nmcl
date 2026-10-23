//#![allow(dead_code)]
use crate::app::{App, ClickTarget, UiLayout};
use crate::worker::ModLoader;
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
    Frame,
};

use super::common::{clickable, BG, GOLD, GRAY, GRAY_DIM, GRASS, SKY, TEXT};

pub fn draw_download_modal(f: &mut Frame, app: &mut App) {
    // 独占点击区
    let mut layout = UiLayout::default();

    let area = centered(f.area(), 60, 18);
    f.render_widget(Clear, area);

    let title = app
        .download_source
        .as_ref()
        .map(|v| format!(" 下载 {} ", v.id))
        .unwrap_or_else(|| " 下载版本 ".into());

    let block = Block::default()
        .title(Line::from(Span::styled(
            title,
            Style::default().fg(GOLD).add_modifier(Modifier::BOLD),
        )))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(GOLD))
        .style(Style::default().bg(BG));
    let inner = block.inner(area);
    f.render_widget(block, area);

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(3),
            Constraint::Min(0),
        ])
        .split(inner);

    // 安装名称
    label_value_row(
        f,
        &mut layout,
        inner,
        rows[1],
        "安装名称",
        &app.download_name_buffer,
        true,
        ClickTarget::DownloadName,
    );

    // 加载器
    let loader_label = format!(" {}  (Tab 切换) ", app.download_loader.title());
    let loader_rect = Rect::new(inner.x, rows[3].y, inner.width, 1);
    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(" 加载器   ", Style::default().fg(GRAY)),
            Span::styled(
                loader_label,
                Style::default()
                    .bg(SKY)
                    .fg(Color::Black)
                    .add_modifier(Modifier::BOLD),
            ),
        ]))
        .style(Style::default().bg(BG)),
        loader_rect,
    );
    clickable(&mut layout, loader_rect, ClickTarget::DownloadLoader);

    // 加载器版本
    let ver_text = if app.download_loader == ModLoader::Vanilla {
        " （原版无需选择）".to_string()
    } else if app.download_fetching_loader_versions {
        " 获取中…".to_string()
    } else if app.download_loader_versions.is_empty() {
        " 无可用版本".to_string()
    } else {
        let cur = app
            .download_loader_ver_index
            .and_then(|i| app.download_loader_versions.get(i))
            .cloned()
            .unwrap_or_else(|| "未选择".into());
        format!(" ↑/↓ {} ", cur)
    };
    let ver_style = if app.download_loader != ModLoader::Vanilla
        && !app.download_loader_versions.is_empty()
    {
        Style::default()
            .bg(GRASS)
            .fg(Color::Black)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().bg(GRAY_DIM).fg(GRAY)
    };
    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(" 加载器版本 ", Style::default().fg(GRAY)),
            Span::styled(ver_text, ver_style),
        ]))
        .style(Style::default().bg(BG)),
        rows[4],
    );

    // 内存
    label_value_row(
        f,
        &mut layout,
        inner,
        rows[6],
        "最大内存(MB)",
        &app.download_memory_buffer,
        false,
        ClickTarget::DownloadMemory,
    );

    // 按钮
    let btn_cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Fill(1),
            Constraint::Length(12),
            Constraint::Length(2),
            Constraint::Length(12),
            Constraint::Fill(1),
        ])
        .split(rows[8]);

    let save_rect = Rect::new(btn_cols[1].x, btn_cols[1].y + 1, btn_cols[1].width, 1);
    let cancel_rect = Rect::new(btn_cols[3].x, btn_cols[3].y + 1, btn_cols[3].width, 1);

    let can_start = app.download_source.is_some()
        && !app.download_name_buffer.trim().is_empty()
        && (app.download_loader == ModLoader::Vanilla
            || !app.download_loader_versions.is_empty());

    let start_style = if can_start {
        Style::default()
            .bg(GRASS)
            .fg(Color::Black)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().bg(GRAY_DIM).fg(GRAY)
    };
    f.render_widget(
        Paragraph::new(Line::from(Span::styled(" 开始下载 ", start_style)))
            .alignment(Alignment::Center)
            .style(start_style),
        save_rect,
    );
    if can_start {
        clickable(&mut layout, save_rect, ClickTarget::ConfirmDownload);
    }

    f.render_widget(
        Paragraph::new(Line::from(Span::styled(
            " 取消 ",
            Style::default().bg(GRAY_DIM).fg(TEXT),
        )))
        .alignment(Alignment::Center)
        .style(Style::default().bg(GRAY_DIM)),
        cancel_rect,
    );
    clickable(&mut layout, cancel_rect, ClickTarget::CancelDownload);

    // 提示
    if inner.height > 14 {
        f.render_widget(
            Paragraph::new(Line::from(Span::styled(
                " 提示: Enter 开始 · Tab 切换加载器 · ↑/↓ 选择版本 · Esc 取消",
                Style::default().fg(GRAY),
            )))
            .style(Style::default().bg(BG)),
            Rect::new(inner.x, inner.y + inner.height - 1, inner.width, 1),
        );
    }

    // 提交点击区
    app.layout = layout;
}

fn label_value_row(
    f: &mut Frame,
    layout: &mut UiLayout,
    parent: Rect,
    row: Rect,
    label: &str,
    value: &str,
    highlight: bool,
    target: ClickTarget,
) {
    let label_rect = Rect::new(parent.x, row.y, 14, 1);
    f.render_widget(
        Paragraph::new(Line::from(Span::styled(
            format!(" {label}"),
            Style::default().fg(GRAY),
        )))
        .style(Style::default().bg(BG)),
        label_rect,
    );
    let vx = parent.x + 14;
    let vw = parent.width.saturating_sub(15);
    if vw == 0 {
        return;
    }
    let vrect = Rect::new(vx, row.y, vw, 1);
    let style = if highlight {
        Style::default().fg(GOLD).bg(Color::Rgb(45, 45, 60))
    } else {
        Style::default().fg(TEXT).bg(Color::Rgb(40, 42, 50))
    };
    f.render_widget(
        Paragraph::new(Line::from(Span::styled(format!(" {value}"), style))).style(style),
        vrect,
    );
    clickable(layout, vrect, target);
    clickable(
        layout,
        Rect::new(parent.x, row.y, parent.width, 1),
        target,
    );
}

fn centered(area: Rect, w: u16, h: u16) -> Rect {
    let w = w.min(area.width.saturating_sub(4));
    let h = h.min(area.height.saturating_sub(2));
    Rect::new(
        area.x + (area.width.saturating_sub(w)) / 2,
        area.y + (area.height.saturating_sub(h)) / 2,
        w,
        h,
    )
}