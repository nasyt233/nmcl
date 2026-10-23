use crate::app::{App, ClickTarget, Tab};
use ratatui::{
    layout::{Alignment, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Gauge, Paragraph},
    Frame,
};
use unicode_width::UnicodeWidthStr;

use super::common::{clickable, BG, DIAMOND, GOLD, GRAY, GRAY_DIM, GRASS, NAV_BG, RED, TEXT};

pub fn draw_nav(f: &mut Frame, app: &mut App, area: Rect) {
    f.render_widget(Block::default().style(Style::default().bg(NAV_BG)), area);
    let mut x = area.x;
    
    // Logo
    let logo = " ⛏ nmcl ";
    let lw = logo.width() as u16;
    f.render_widget(
        Paragraph::new(Line::from(Span::styled(
            logo,
            Style::default().bg(GRASS).fg(Color::Black).add_modifier(Modifier::BOLD),
        ))),
        Rect::new(x, area.y, lw, 1),
    );
    x += lw;

    // 导航页签
    for tab in [Tab::Home, Tab::Versions, Tab::Downloads, Tab::Settings] {
        let title = format!(" {} ", tab.title());
        let w = title.width() as u16;
        let rect = Rect::new(x, area.y, w, 1);
        let style = if app.tab == tab {
            Style::default().bg(GRASS).fg(Color::Black).add_modifier(Modifier::BOLD)
        } else {
            Style::default().bg(NAV_BG).fg(TEXT)
        };
        f.render_widget(Paragraph::new(Line::from(Span::styled(title, style))), rect);
        clickable(&mut app.layout, rect, ClickTarget::Tab(tab));
        x += w + 1;
    }

    // 右侧状态胶囊
    let (txt, color) = if app.status == "错误" {
        (" ✕ 错误 ", RED)
    } else if app.status == "取消中" {
        (" ◌ 取消中 ", GOLD)
    } else if app.busy {
        (" ◆ 工作中 ", DIAMOND)
    } else {
        (" ✓ 就绪 ", GRASS)
    };
    let sw = txt.width() as u16;
    let rx = area.x + area.width.saturating_sub(sw + 1);
    f.render_widget(
        Paragraph::new(Line::from(Span::styled(
            txt,
            Style::default().bg(color).fg(Color::Black).add_modifier(Modifier::BOLD),
        ))),
        Rect::new(rx, area.y, sw, 1),
    );
}

pub fn draw_progress(f: &mut Frame, app: &App, area: Rect) {
    f.render_widget(Block::default().style(Style::default().bg(NAV_BG)), area);
    if let Some((done, total, label)) = &app.progress {
        if *total > 0 {
            let ratio = (*done as f64 / *total as f64).clamp(0.0, 1.0);
            
            // 将后端的 label 转为中文
            let cn_label = match label.as_str() {
                "libraries" => "依赖库",
                "assets" => "游戏资源",
                "modpack" => "整合包",
                "resource" => "资源文件",
                _ => label.as_str(),
            };
            
            let text = format!(" 正在下载 {}  {}/{}  ({:.0}%) ", cn_label, done, total, ratio * 100.0);
            f.render_widget(
                Gauge::default()
                    .gauge_style(Style::default().fg(DIAMOND).bg(GRAY_DIM))
                    .ratio(ratio)
                    .label(Span::styled(text, Style::default().fg(Color::Black))),
                area,
            );
            return;
        }
    }
}

pub fn draw_logs(f: &mut Frame, app: &App, area: Rect) {
    let h = area.height as usize;
    let start = app.logs.len().saturating_sub(h);
    
    let lines: Vec<Line> = app.logs[start..]
        .iter()
        .map(|l| {
            // 1. 判断日志级别和着色
            let (prefix, style) = if l.contains("错误") || l.contains("失败") || l.starts_with("[MC!]") || l.starts_with("[错误]") {
                ("[错误] ", Style::default().fg(RED).add_modifier(Modifier::BOLD))
            } else if l.contains("完成") || l.contains("成功") || l.starts_with("[完成]") {
                ("[完成] ", Style::default().fg(GRASS))
            } else if l.starts_with("[MC]") || l.starts_with("[游戏]") {
                ("[游戏] ", Style::default().fg(DIAMOND))
            } else if l.contains("进度") || l.contains("正在") || l.contains("下载") || l.starts_with("[进度]") || l.starts_with("[下载]") {
                ("[进度] ", Style::default().fg(super::common::SKY))
            } else {
                ("[信息] ", Style::default().fg(GRAY))
            };
            
            // 2. 清理所有可能存在的旧标签前缀
            let mut msg = l
                .replace("[MC] ", "")
                .replace("[MC!] ", "")
                .replace("[下载] ", "")
                .replace("[进度] ", "")
                .replace("[信息] ", "")
                .replace("[完成] ", "")
                .replace("[错误] ", "")
                .replace("[警告] ", "");
            
            // 3. 根据终端宽度截断，防止折行挤占其他日志的显示空间
            let max_chars = (area.width as usize).saturating_sub(10);
            if msg.chars().count() > max_chars && max_chars > 3 {
                msg = msg.chars().take(max_chars - 3).collect::<String>() + "...";
            }

            Line::from(vec![
                Span::styled(prefix, style),
                Span::styled(msg, Style::default().fg(TEXT)),
            ])
        })
        .collect();

    f.render_widget(
        Paragraph::new(lines)
            .style(Style::default().bg(BG))
            .block(
                Block::default()
                    .title(" 控制台 ")
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(GRAY_DIM)),
            ),
        area,
    );
}

pub fn draw_help(f: &mut Frame, area: Rect) {
    let help = Line::from(vec![
        Span::styled(" 1/2/3/4 ", Style::default().bg(GRAY_DIM).fg(GOLD)), 
        Span::styled(" 切页  ", Style::default().fg(TEXT)),
        Span::styled(" ↑/↓ ", Style::default().bg(GRAY_DIM).fg(GOLD)), 
        Span::styled(" 选择  ", Style::default().fg(TEXT)),
        Span::styled(" Enter ", Style::default().bg(GRAY_DIM).fg(GOLD)), 
        Span::styled(" 确认/启动  ", Style::default().fg(TEXT)),
        Span::styled(" [ ] ", Style::default().bg(GRAY_DIM).fg(GOLD)), 
        Span::styled(" 翻页  ", Style::default().fg(TEXT)),
        Span::styled(" Esc ", Style::default().bg(GRAY_DIM).fg(GOLD)), 
        Span::styled(" 返回  ", Style::default().fg(TEXT)),
        Span::styled(" q ", Style::default().bg(GRAY_DIM).fg(GOLD)), 
        Span::styled(" 退出 ", Style::default().fg(TEXT)),
    ]);
    f.render_widget(
        Paragraph::new(help)
            .alignment(Alignment::Center)
            .style(Style::default().bg(NAV_BG)), 
        area
    );
}