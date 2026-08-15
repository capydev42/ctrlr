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

/// Columns the command list keeps for itself before a side pane may have any.
pub const MIN_LIST_WIDTH: u16 = 24;

/// Narrowest a side pane may be and still be worth drawing. `render_details`
/// gives up under 5 columns, so this has to stay comfortably above that or a
/// pane would silently vanish instead of shrinking.
pub const MIN_SIDE_WIDTH: u16 = 16;

/// The horizontal split of the content row.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ContentAreas {
    pub collections: Option<Rect>,
    pub list: Rect,
    pub details: Option<Rect>,
    /// Grab targets for resizing, indexed by [`crate::app::Divider`]. Each is
    /// the two-column seam where one pane's border meets the next one's; a
    /// pane that is not shown leaves a zero-sized rect, which hit-tests false.
    pub dividers: [Rect; 2],
}

/// Seam between a pane ending at `left_end` and the pane starting after it.
fn seam(left_pane: Rect) -> Rect {
    Rect::new(
        left_pane.x + left_pane.width.saturating_sub(1),
        left_pane.y,
        2,
        left_pane.height,
    )
}

/// Lays out the content row from requested side-pane widths in **columns**.
///
/// Widths are columns rather than percentages so a drag maps 1:1 onto them —
/// a percentage round-trip lands on the same percent for small movements and
/// makes the divider stick. The cost is that widening the terminal grows only
/// the list, which is why the side panes are clamped here on every frame
/// rather than trusted as stored.
///
/// `None` means the pane is not shown. A pane that cannot keep
/// [`MIN_SIDE_WIDTH`] without pushing the list under [`MIN_LIST_WIDTH`] is
/// trimmed, and dropped entirely only when trimming is not enough — details
/// first, since the list it describes matters more than the description.
pub fn split_content(area: Rect, collections: Option<u16>, details: Option<u16>) -> ContentAreas {
    let mut coll = collections.map(|w| w.max(MIN_SIDE_WIDTH));
    let mut det = details.map(|w| w.max(MIN_SIDE_WIDTH));

    // Keep both panes if trimming can pay for the list. If it cannot, drop
    // details and start over from the *requested* collections width — a pane
    // on its way out should not leave its neighbour shrunken behind it.
    if !trim_to_fit(area.width, &mut coll, &mut det) {
        det = None;
        coll = collections.map(|w| w.max(MIN_SIDE_WIDTH));
        if !trim_to_fit(area.width, &mut coll, &mut det) {
            coll = None;
        }
    }

    let coll_width = coll.unwrap_or(0);
    let det_width = det.unwrap_or(0);
    let list_width = area.width.saturating_sub(coll_width + det_width);

    let collections = coll.map(|w| Rect::new(area.x, area.y, w, area.height));
    let list = Rect::new(area.x + coll_width, area.y, list_width, area.height);
    let details = det.map(|w| Rect::new(area.x + coll_width + list_width, area.y, w, area.height));

    ContentAreas {
        collections,
        list,
        details,
        dividers: [
            collections.map(seam).unwrap_or_default(),
            details.map(|_| seam(list)).unwrap_or_default(),
        ],
    }
}

/// Columns the side panes must give back for the list to keep its minimum.
fn shortfall(total: u16, collections: Option<u16>, details: Option<u16>) -> u16 {
    let side = collections.unwrap_or(0) + details.unwrap_or(0);
    (side + MIN_LIST_WIDTH).saturating_sub(total)
}

/// Shrinks the side panes toward [`MIN_SIDE_WIDTH`], details first, until the
/// list has its minimum. False when even the minimums do not fit.
fn trim_to_fit(total: u16, collections: &mut Option<u16>, details: &mut Option<u16>) -> bool {
    let mut over = shortfall(total, *collections, *details);
    for pane in [details, collections] {
        if over == 0 {
            break;
        }
        if let Some(w) = pane.as_mut() {
            let give = over.min(w.saturating_sub(MIN_SIDE_WIDTH));
            *w -= give;
            over -= give;
        }
    }
    over == 0
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
    /// The content row the panes are laid out in — a drag turns a column
    /// inside it into a pane width, so it needs the row's own bounds.
    pub content: Rect,
    /// Grab targets for the resizable seams, indexed by
    /// [`crate::app::Divider`].
    pub dividers: [Rect; 2],
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
    fn test_layout_split_content_honours_requested_widths() {
        let areas = split_content(Rect::new(0, 4, 100, 20), None, Some(35));
        assert_eq!(areas.collections, None);
        assert_eq!(areas.list, Rect::new(0, 4, 65, 20));
        assert_eq!(areas.details, Some(Rect::new(65, 4, 35, 20)));
    }

    #[test]
    fn test_layout_split_content_three_panes_are_adjacent() {
        let areas = split_content(Rect::new(2, 4, 120, 20), Some(24), Some(35));
        let coll = areas.collections.unwrap();
        let det = areas.details.unwrap();
        assert_eq!(coll, Rect::new(2, 4, 24, 20));
        assert_eq!(areas.list, Rect::new(26, 4, 61, 20));
        assert_eq!(det, Rect::new(87, 4, 35, 20));
        assert_eq!(coll.width + areas.list.width + det.width, 120);
    }

    #[test]
    fn test_layout_split_content_raises_undersized_request() {
        let areas = split_content(Rect::new(0, 0, 100, 10), None, Some(3));
        assert_eq!(areas.details.unwrap().width, MIN_SIDE_WIDTH);
    }

    #[test]
    fn test_layout_split_content_trims_details_before_collections() {
        // 24 + 60 leaves the list 16, under its minimum: details gives back 8.
        let areas = split_content(Rect::new(0, 0, 100, 10), Some(24), Some(60));
        assert_eq!(areas.collections.unwrap().width, 24);
        assert_eq!(areas.details.unwrap().width, 52);
        assert_eq!(areas.list.width, MIN_LIST_WIDTH);
    }

    #[test]
    fn test_layout_split_content_drops_details_when_trimming_is_not_enough() {
        // 24 + 16 + 24 needs 64 columns; 55 cannot hold all three.
        let areas = split_content(Rect::new(0, 0, 55, 10), Some(24), Some(20));
        assert_eq!(areas.details, None);
        assert_eq!(areas.collections.unwrap().width, 24);
        assert_eq!(areas.list.width, 31);
    }

    #[test]
    fn test_layout_split_content_drops_both_on_a_tiny_terminal() {
        let areas = split_content(Rect::new(0, 0, 30, 10), Some(24), Some(20));
        assert_eq!(areas.collections, None);
        assert_eq!(areas.details, None);
        assert_eq!(areas.list, Rect::new(0, 0, 30, 10));
    }

    #[test]
    fn test_layout_split_content_dividers_sit_on_the_seams() {
        let areas = split_content(Rect::new(0, 4, 120, 20), Some(24), Some(35));
        // Collections spans 0..24, so its right border is column 23 and the
        // list's left border is 24.
        assert_eq!(areas.dividers[0], Rect::new(23, 4, 2, 20));
        // The list spans 24..85, so its right border is 84 and details' left
        // border is 85.
        assert_eq!(areas.details.unwrap().x, 85);
        assert_eq!(areas.dividers[1], Rect::new(84, 4, 2, 20));
    }

    #[test]
    fn test_layout_split_content_hidden_panes_have_no_divider() {
        let areas = split_content(Rect::new(0, 0, 100, 10), None, Some(30));
        assert_eq!(areas.dividers[0], Rect::default());
        assert!(!hits(areas.dividers[0], 0, 0));
        assert_ne!(areas.dividers[1], Rect::default());

        // A pane dropped for lack of room loses its grab target too.
        let cramped = split_content(Rect::new(0, 0, 30, 10), Some(24), Some(20));
        assert_eq!(cramped.dividers, [Rect::default(), Rect::default()]);
    }

    #[test]
    fn test_layout_split_content_without_side_panes() {
        let areas = split_content(Rect::new(0, 0, 80, 10), None, None);
        assert_eq!(areas.list, Rect::new(0, 0, 80, 10));
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
