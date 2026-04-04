use std::time::Instant;

use ratatui::layout::Position;

use super::render::Rects;
use super::session::SidebarItem;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MouseAction {
    SelectSidebar(usize),
    ClickTaskPane { row: usize },
    Scroll { in_task_pane: bool, delta: isize },
}

pub fn resolve_mouse_action(
    col: u16,
    row: u16,
    rects: &Rects,
    sidebar_items: &[SidebarItem],
) -> Option<MouseAction> {
    let pos = Position::new(col, row);

    // Check sidebar
    let sb = &rects.sidebar;
    if sb.contains(pos) {
        // Must be inside border (not on border row)
        if row <= sb.y || row >= sb.y.saturating_add(sb.height).saturating_sub(1) {
            return None;
        }
        let inner_row = (row - sb.y - 1) as usize;
        let index = inner_row + rects.sidebar_offset;
        if index >= sidebar_items.len() {
            return None;
        }
        return match &sidebar_items[index] {
            SidebarItem::Separator | SidebarItem::ProjectsHeader | SidebarItem::ContextsHeader => {
                None
            }
            _ => Some(MouseAction::SelectSidebar(index)),
        };
    }

    // Check task pane
    let tp = &rects.task_pane;
    if tp.contains(pos) {
        if row <= tp.y || row >= tp.y.saturating_add(tp.height).saturating_sub(1) {
            return None;
        }
        let inner_row = (row - tp.y - 1) as usize;
        return Some(MouseAction::ClickTaskPane { row: inner_row });
    }

    None
}

pub fn resolve_scroll_action(
    col: u16,
    row: u16,
    rects: &Rects,
    delta: isize,
) -> Option<MouseAction> {
    let pos = Position::new(col, row);

    if rects.sidebar.contains(pos) {
        return Some(MouseAction::Scroll {
            in_task_pane: false,
            delta,
        });
    }

    if rects.task_pane.contains(pos) {
        return Some(MouseAction::Scroll {
            in_task_pane: true,
            delta,
        });
    }

    None
}

pub struct DoubleClickTracker {
    last_click: Option<(Instant, usize)>,
}

impl DoubleClickTracker {
    pub fn new() -> Self {
        Self { last_click: None }
    }

    /// Returns true on double-click (same task within 500ms). Resets state after double-click.
    pub fn record(&mut self, task_index: usize) -> bool {
        if let Some((instant, prev_index)) = self.last_click.take() {
            if prev_index == task_index && instant.elapsed().as_millis() < 500 {
                return true;
            }
        }
        self.last_click = Some((Instant::now(), task_index));
        false
    }
}
