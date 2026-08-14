use ratatui::layout::{Position, Rect};

pub fn center_rect(width: u16, height: u16, area: Rect) -> Rect {
    let vertical = Rect::new(
        area.x,
        area.y + (area.height.saturating_sub(height)) / 2,
        area.width.min(width),
        height.min(area.height),
    );
    Rect::new(
        area.x + (area.width.saturating_sub(width)) / 2,
        vertical.y,
        vertical.width,
        vertical.height,
    )
}

/// Screen regions recorded during the last draw, for mouse hit-testing.
///
/// `ui::render` runs immediately before every blocking `event::read()`, so
/// whatever the last frame recorded is exactly what the user clicked on.
/// Reset to the default at the top of every frame: an unset region is
/// zero-sized, and `Rect::contains` is false for those, so stale hitboxes
/// cannot survive a view change.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Hitboxes {
    pub search: Rect,
    /// History, Favorites, Collections — in tab order.
    pub tabs: [Rect; 3],
    /// The command list: history, favorites, or a collection's commands.
    pub list: Rect,
    pub details: Rect,
    pub collections_list: Rect,
    /// Outer rect of the topmost popup; a click outside it dismisses.
    pub popup: Rect,
    pub context_menu: Rect,
}

/// Item index under `y` for a bordered list, or `None` outside its inner area.
///
/// `offset` is the list's scroll offset (`ListState::offset`) and `len` the
/// number of real items — lists render a placeholder row when empty, and that
/// row must not hit-test as item 0.
pub fn row_index(area: Rect, offset: usize, y: u16, len: usize) -> Option<usize> {
    if len == 0 {
        return None;
    }
    let inner_top = area.y.checked_add(1)?;
    let inner_bottom = area.y.saturating_add(area.height).checked_sub(1)?;
    if y < inner_top || y >= inner_bottom {
        return None;
    }
    let index = offset + (y - inner_top) as usize;
    (index < len).then_some(index)
}

/// Place a `width`×`height` rect at `(x, y)`, flipped and then clamped to stay
/// inside `area`. Used to anchor the context menu at the pointer without
/// letting it run off the bottom or right edge.
pub fn anchor_rect(x: u16, y: u16, width: u16, height: u16, area: Rect) -> Rect {
    let width = width.min(area.width);
    let height = height.min(area.height);

    // Flip above/left of the pointer when there is no room after it, so the
    // menu never covers the row it was opened on unless it has to.
    let mut left = if x + width > area.x + area.width {
        x.saturating_sub(width)
    } else {
        x
    };
    let mut top = if y + height > area.y + area.height {
        y.saturating_sub(height)
    } else {
        y
    };

    left = left.clamp(area.x, (area.x + area.width).saturating_sub(width));
    top = top.clamp(area.y, (area.y + area.height).saturating_sub(height));

    Rect::new(left, top, width, height)
}

/// Whether `(x, y)` falls inside `rect`.
pub fn hits(rect: Rect, x: u16, y: u16) -> bool {
    rect.contains(Position::new(x, y))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn area() -> Rect {
        Rect::new(0, 0, 40, 12)
    }

    #[test]
    fn test_layout_row_index_skips_border() {
        // y == area.y is the top border, so the first item sits one row down.
        assert_eq!(row_index(area(), 0, 0, 5), None);
        assert_eq!(row_index(area(), 0, 1, 5), Some(0));
        assert_eq!(row_index(area(), 0, 2, 5), Some(1));
    }

    #[test]
    fn test_layout_row_index_skips_bottom_border() {
        // Height 12 means rows 0..=11; row 11 is the bottom border.
        assert_eq!(row_index(area(), 0, 10, 20), Some(9));
        assert_eq!(row_index(area(), 0, 11, 20), None);
        assert_eq!(row_index(area(), 0, 30, 20), None);
    }

    #[test]
    fn test_layout_row_index_applies_offset() {
        assert_eq!(row_index(area(), 7, 1, 20), Some(7));
        assert_eq!(row_index(area(), 7, 3, 20), Some(9));
    }

    #[test]
    fn test_layout_row_index_empty_list() {
        // The "No results found" placeholder must not hit-test as item 0.
        assert_eq!(row_index(area(), 0, 1, 0), None);
    }

    #[test]
    fn test_layout_row_index_past_last_item() {
        assert_eq!(row_index(area(), 0, 3, 2), None);
    }

    #[test]
    fn test_layout_row_index_offset_area() {
        let rect = Rect::new(5, 4, 20, 6);
        assert_eq!(row_index(rect, 0, 4, 10), None);
        assert_eq!(row_index(rect, 0, 5, 10), Some(0));
        assert_eq!(row_index(rect, 0, 9, 10), None);
    }

    #[test]
    fn test_layout_anchor_rect_fits() {
        assert_eq!(anchor_rect(3, 2, 10, 4, area()), Rect::new(3, 2, 10, 4));
    }

    #[test]
    fn test_layout_anchor_rect_flips_at_edges() {
        let r = anchor_rect(38, 11, 10, 4, area());
        assert_eq!(r, Rect::new(28, 7, 10, 4));
    }

    #[test]
    fn test_layout_anchor_rect_clamps_when_oversized() {
        let r = anchor_rect(30, 8, 80, 40, area());
        assert_eq!(r, area());
    }

    #[test]
    fn test_layout_hits() {
        let rect = Rect::new(2, 2, 4, 4);
        assert!(hits(rect, 2, 2));
        assert!(hits(rect, 5, 5));
        assert!(!hits(rect, 6, 5));
        assert!(!hits(rect, 1, 2));
        assert!(!hits(Rect::default(), 0, 0));
    }
}
