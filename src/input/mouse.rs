use std::time::{Duration, Instant};

use crossterm::event::{MouseButton, MouseEvent, MouseEventKind};

use crate::app::{Action, ActivePane, AppState, ContextMenuItem, Divider, InputMode, ViewMode};
use crate::input::normal;
use crate::keymap::KeyAction;
use crate::ui::layout::{hits, row_index};

/// How long after a click a second click on the same row counts as a
/// double-click. Crossterm reports no double-click of its own. Matches the
/// 500ms most desktops default to; shorter feels broken to slower hands.
const DOUBLE_CLICK: Duration = Duration::from_millis(500);

/// Rows the wheel moves the selection by per notch.
const WHEEL_ROWS: isize = 3;

/// Turns a mouse event into the same [`Action`] the equivalent key would
/// produce. Hit-testing uses the rects recorded by the previous draw
/// (`AppState::hit`), which is always the frame the user is looking at.
pub fn handle(state: &mut AppState, ev: MouseEvent) -> Action {
    // A live drag owns the mouse: it consumes the motion events the filter
    // below would otherwise throw away, and swallows everything else so a
    // divider drag can never also select a row. Ahead of the filter for that
    // reason.
    if let Some(divider) = state.dragging {
        match ev.kind {
            MouseEventKind::Drag(MouseButton::Left) => state.resize_divider(divider, ev.column),
            MouseEventKind::Up(_) => {
                state.dragging = None;
                // Once per drag rather than per motion event.
                state.persist_pane_widths();
            }
            _ => {}
        }
        return Action::None;
    }

    // `EnableMouseCapture` turns on motion tracking too, and it has to stay on
    // (see `run_tui`). Nothing here reacts to hover, so drop these first.
    if matches!(
        ev.kind,
        MouseEventKind::Moved | MouseEventKind::Drag(_) | MouseEventKind::Up(_)
    ) {
        return Action::None;
    }

    let (x, y) = (ev.column, ev.row);

    if state.context_menu_open {
        return handle_context_menu(state, ev, x, y);
    }

    // The integration popup is the one modal a stray click must not answer:
    // dismissing it records a decline. Swallow everything instead.
    if state.integration_popup_open {
        return Action::None;
    }

    if popup_open(state) {
        return handle_popup(state, ev, x, y);
    }

    match ev.kind {
        MouseEventKind::ScrollDown => scroll(state, WHEEL_ROWS, x, y),
        MouseEventKind::ScrollUp => scroll(state, -WHEEL_ROWS, x, y),
        MouseEventKind::Down(MouseButton::Left) => return click(state, x, y),
        MouseEventKind::Down(MouseButton::Right) => right_click(state, x, y),
        _ => {}
    }

    Action::None
}

/// Derived from the context chain rather than kept as a third hand-maintained
/// copy of the overlay list — `input::active_context` and
/// `AppState::cancel_or_quit` are the other two, and a new overlay missing from
/// this one used to mean the wheel and click-outside silently did nothing.
fn popup_open(state: &AppState) -> bool {
    super::active_context(state).is_overlay()
}

/// Modal popups are driven through the same dispatcher their keys use, rather
/// than by poking at popup state — so the two routes cannot drift.
///
/// This names *actions*, not keys. It used to synthesize `KeyCode::Down` and
/// `KeyCode::Esc` and feed them back through the key handler, which stops being
/// correct the moment those keys are rebindable: the wheel would follow the
/// user's `navigate_down` binding around, or stop working when they moved it.
///
/// Rows inside popups stay keyboard-driven — the help and tag lists interleave
/// headers and a "create" entry, so a rendered row is not a selection index.
fn handle_popup(state: &mut AppState, ev: MouseEvent, x: u16, y: u16) -> Action {
    let context = super::active_context(state);
    match ev.kind {
        MouseEventKind::ScrollDown => super::dispatch(state, context, KeyAction::NavigateDown),
        MouseEventKind::ScrollUp => super::dispatch(state, context, KeyAction::NavigateUp),
        MouseEventKind::Down(MouseButton::Left) if !hits(state.hit.popup, x, y) => {
            super::dispatch(state, context, KeyAction::Cancel)
        }
        _ => Action::None,
    }
}

fn handle_context_menu(state: &mut AppState, ev: MouseEvent, x: u16, y: u16) -> Action {
    match ev.kind {
        MouseEventKind::ScrollDown => {
            state.navigate_context_menu(1);
            Action::None
        }
        MouseEventKind::ScrollUp => {
            state.navigate_context_menu(-1);
            Action::None
        }
        MouseEventKind::Down(_) => {
            let len = state.context_menu_items.len();
            let row = row_index(state.hit.context_menu, 0, y, len)
                .filter(|_| hits(state.hit.context_menu, x, y));
            match row {
                Some(row) => {
                    state.select_context_menu_index(row);
                    activate_context_menu(state)
                }
                // A click anywhere else closes the menu without acting, the
                // way a menu is expected to behave.
                None => {
                    state.close_context_menu();
                    Action::None
                }
            }
        }
        _ => Action::None,
    }
}

/// Runs the selected menu entry through the same state calls its keybinding
/// uses, so the two routes cannot drift.
pub fn activate_context_menu(state: &mut AppState) -> Action {
    let Some(item) = state
        .context_menu_items
        .get(state.context_menu_index)
        .copied()
    else {
        state.close_context_menu();
        return Action::None;
    };
    state.close_context_menu();

    match item {
        ContextMenuItem::Execute => normal::activate_selected(state),
        ContextMenuItem::Copy => {
            let text = state
                .filtered
                .get(state.selected_index)
                .map(|c| c.text.clone());
            if let Some(text) = text {
                let (success, msg) = crate::app::clipboard::copy_to_clipboard(&text);
                if success {
                    state.set_status_message("📋 Copied to clipboard".into());
                } else if let Some(msg) = msg {
                    state.set_status_message(msg);
                }
            }
            Action::None
        }
        ContextMenuItem::ToggleFavorite => {
            state.toggle_favorite();
            Action::None
        }
        ContextMenuItem::AddTag => {
            state.input_mode = InputMode::TagInput;
            state.tag_input = String::new();
            state.tag_selected_index = 0;
            state.tag_cursor_index = None;
            Action::None
        }
        ContextMenuItem::AddToCollection => {
            state.collection_input_mode = crate::app::CollectionInputMode::AddToCollection;
            state.input_mode = InputMode::CollectionInput;
            Action::None
        }
        ContextMenuItem::RemoveFromCollection => {
            if let Some(text) = state
                .filtered
                .get(state.selected_index)
                .map(|c| c.text.clone())
            {
                state.remove_command_from_collection(&text);
            }
            Action::None
        }
        // Enter on the collections pane drills into the collection, which is
        // what "Open" means here.
        ContextMenuItem::OpenCollection => {
            state.active_pane = ActivePane::CollectionsList;
            normal::activate_selected(state)
        }
        ContextMenuItem::NewCollection => {
            state.begin_new_collection();
            Action::None
        }
        ContextMenuItem::RenameCollection => {
            state.begin_rename_collection();
            Action::None
        }
        ContextMenuItem::DeleteCollection => {
            // Opens the same confirm popup `d` does; nothing is removed until
            // the user answers it.
            state.delete_collection();
            Action::None
        }
    }
}

fn scroll(state: &mut AppState, delta: isize, x: u16, y: u16) {
    if hits(state.hit.collections_list, x, y) {
        state.scroll_collections(delta);
    } else if hits(state.hit.list, x, y) {
        state.scroll_list(delta);
    }
}

fn click(state: &mut AppState, x: u16, y: u16) -> Action {
    // Checked before the panes: the seam sits on their borders, so whichever
    // is tested first wins, and grabbing a divider must not also select.
    for (i, divider) in [Divider::Collections, Divider::Details]
        .into_iter()
        .enumerate()
    {
        if hits(state.hit.dividers[i], x, y) {
            state.dragging = Some(divider);
            state.last_click = None;
            return Action::None;
        }
    }

    if hits(state.hit.search, x, y) {
        state.active_pane = ActivePane::Search;
        state.last_click = None;
        return Action::None;
    }

    for (i, tab) in state.hit.tabs.iter().enumerate() {
        if hits(*tab, x, y) {
            match i {
                0 => normal::switch_view_history(state),
                1 => normal::switch_view_favorites(state),
                _ => normal::switch_view_collections(state),
            }
            state.last_click = None;
            return Action::None;
        }
    }

    if hits(state.hit.collections_list, x, y) {
        let row = row_index(
            state.hit.collections_list,
            state.collection_list_state.offset(),
            y,
            state.collections.len(),
        );
        if let Some(row) = row {
            state.active_pane = ActivePane::CollectionsList;
            state.select_collection_index(row);
            if double_click(state, x, y) {
                return normal::activate_selected(state);
            }
        }
        return Action::None;
    }

    if hits(state.hit.list, x, y) {
        let offset = if state.view_mode == ViewMode::Collections {
            state.collection_items_list_state.offset()
        } else {
            state.list_state.offset()
        };
        let row = row_index(state.hit.list, offset, y, state.filtered.len());
        if let Some(row) = row {
            state.active_pane = if state.view_mode == ViewMode::Collections {
                ActivePane::CollectionItems
            } else {
                ActivePane::History
            };
            state.select_index(row);
            if double_click(state, x, y) {
                return normal::activate_selected(state);
            }
        }
        return Action::None;
    }

    Action::None
}

fn right_click(state: &mut AppState, x: u16, y: u16) {
    // The collections pane gets its own menu: it acts on the collection, not
    // on a command inside it.
    if hits(state.hit.collections_list, x, y) {
        state.active_pane = ActivePane::CollectionsList;
        let row = row_index(
            state.hit.collections_list,
            state.collection_list_state.offset(),
            y,
            state.collections.len(),
        );
        // No row under the pointer still opens the menu, so an empty pane can
        // offer "New collection".
        if let Some(row) = row {
            state.select_collection_index(row);
        }
        state.open_collection_context_menu(x, y);
        return;
    }

    if !hits(state.hit.list, x, y) {
        return;
    }
    let offset = if state.view_mode == ViewMode::Collections {
        state.collection_items_list_state.offset()
    } else {
        state.list_state.offset()
    };
    let Some(row) = row_index(state.hit.list, offset, y, state.filtered.len()) else {
        return;
    };
    state.active_pane = if state.view_mode == ViewMode::Collections {
        ActivePane::CollectionItems
    } else {
        ActivePane::History
    };
    state.select_index(row);
    state.open_context_menu(x, y);
}

/// Records this click and reports whether it completes a double-click on the
/// same row. The pair is consumed, so a third click starts a new one.
///
/// Deliberately compares the row and not the exact cell: a hand moves the
/// pointer a column or two between clicks, and both clicks still mean the same
/// list item. Requiring an identical cell makes double-click feel broken.
fn double_click(state: &mut AppState, x: u16, y: u16) -> bool {
    let now = Instant::now();
    let is_double = matches!(
        state.last_click,
        Some((_, py, at)) if py == y && now.duration_since(at) < DOUBLE_CLICK
    );
    state.last_click = if is_double { None } else { Some((x, y, now)) };
    is_double
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::Command;
    use crossterm::event::KeyModifiers;
    use ratatui::layout::Rect;

    fn command(text: &str) -> Command {
        Command {
            id: crate::hash::hash_command(text),
            text: text.to_string(),
            tags: Vec::new(),
            collection_ids: Vec::new(),
            favorite: false,
            _context: Vec::new(),
            use_count: 0,
            last_used: None,
            runs_here: 0,
        }
    }

    /// A state whose list occupies rows 1..=8 of a 10-row bordered box, with
    /// the tabs on row 0 of their own strip.
    fn state_with(texts: &[&str]) -> AppState {
        let commands: Vec<Command> = texts.iter().map(|t| command(t)).collect();
        let mut state = AppState::new(commands, None);
        state.hit.list = Rect::new(0, 10, 40, 10);
        state.hit.search = Rect::new(0, 0, 40, 3);
        state.hit.tabs = [
            Rect::new(0, 3, 10, 1),
            Rect::new(10, 3, 10, 1),
            Rect::new(20, 3, 10, 1),
        ];
        state
    }

    fn ev(kind: MouseEventKind, x: u16, y: u16) -> MouseEvent {
        MouseEvent {
            kind,
            column: x,
            row: y,
            modifiers: KeyModifiers::NONE,
        }
    }

    fn left(x: u16, y: u16) -> MouseEvent {
        ev(MouseEventKind::Down(MouseButton::Left), x, y)
    }

    fn right(x: u16, y: u16) -> MouseEvent {
        ev(MouseEventKind::Down(MouseButton::Right), x, y)
    }

    #[test]
    fn test_mouse_click_selects_row() {
        let mut state = state_with(&["a", "b", "c"]);
        assert_eq!(handle(&mut state, left(5, 13)), Action::None);
        assert_eq!(state.selected_index, 2);
        assert_eq!(state.list_state.selected(), Some(2));
        assert_eq!(state.active_pane, ActivePane::History);
    }

    #[test]
    fn test_mouse_click_on_border_keeps_selection() {
        let mut state = state_with(&["a", "b", "c"]);
        state.select_index(1);
        handle(&mut state, left(5, 10));
        assert_eq!(state.selected_index, 1);
    }

    #[test]
    fn test_mouse_click_past_last_item_keeps_selection() {
        let mut state = state_with(&["a", "b"]);
        handle(&mut state, left(5, 17));
        assert_eq!(state.selected_index, 0);
    }

    #[test]
    fn test_mouse_double_click_executes() {
        let mut state = state_with(&["ls -la", "git status"]);
        assert_eq!(handle(&mut state, left(5, 12)), Action::None);
        assert_eq!(
            handle(&mut state, left(5, 12)),
            Action::Execute("git status".into())
        );
    }

    #[test]
    fn test_mouse_second_click_on_another_row_is_not_a_double_click() {
        let mut state = state_with(&["ls -la", "git status"]);
        handle(&mut state, left(5, 12));
        assert_eq!(handle(&mut state, left(5, 11)), Action::None);
    }

    #[test]
    fn test_mouse_double_click_tolerates_pointer_drift() {
        // A hand moves a column or two between clicks; same row, same item.
        let mut state = state_with(&["ls -la", "git status"]);
        handle(&mut state, left(5, 12));
        assert_eq!(
            handle(&mut state, left(9, 12)),
            Action::Execute("git status".into())
        );
    }

    #[test]
    fn test_mouse_third_click_is_not_a_double_click() {
        let mut state = state_with(&["ls -la", "git status"]);
        handle(&mut state, left(5, 12));
        assert!(matches!(
            handle(&mut state, left(5, 12)),
            Action::Execute(_)
        ));
        // The pair was consumed, so the next click starts over.
        assert_eq!(handle(&mut state, left(5, 12)), Action::None);
    }

    #[test]
    fn test_mouse_wheel_scrolls_without_wrapping() {
        let mut state = state_with(&["a", "b", "c", "d", "e"]);
        handle(&mut state, ev(MouseEventKind::ScrollUp, 5, 12));
        assert_eq!(state.selected_index, 0);

        handle(&mut state, ev(MouseEventKind::ScrollDown, 5, 12));
        assert_eq!(state.selected_index, 3);
        handle(&mut state, ev(MouseEventKind::ScrollDown, 5, 12));
        assert_eq!(state.selected_index, 4);
        assert_eq!(state.list_state.selected(), Some(4));
    }

    #[test]
    fn test_mouse_wheel_outside_the_list_does_nothing() {
        let mut state = state_with(&["a", "b", "c", "d", "e"]);
        handle(&mut state, ev(MouseEventKind::ScrollDown, 5, 1));
        assert_eq!(state.selected_index, 0);
    }

    #[test]
    fn test_mouse_click_on_search_focuses_it() {
        let mut state = state_with(&["a"]);
        state.active_pane = ActivePane::History;
        handle(&mut state, left(4, 1));
        assert_eq!(state.active_pane, ActivePane::Search);
    }

    #[test]
    fn test_mouse_click_on_tab_switches_view() {
        let mut state = state_with(&["a"]);
        handle(&mut state, left(12, 3));
        assert_eq!(state.view_mode, ViewMode::Favorites);
    }

    #[test]
    fn test_mouse_motion_is_ignored() {
        let mut state = state_with(&["a", "b"]);
        state.select_index(1);
        handle(&mut state, ev(MouseEventKind::Moved, 5, 11));
        assert_eq!(state.selected_index, 1);
    }

    #[test]
    fn test_mouse_right_click_opens_menu_on_that_row() {
        let mut state = state_with(&["a", "b", "c"]);
        handle(&mut state, right(5, 13));
        assert!(state.context_menu_open);
        assert_eq!(state.selected_index, 2);
        assert_eq!(state.context_menu_pos, (5, 13));
        assert_eq!(
            state.context_menu_items,
            vec![
                ContextMenuItem::Execute,
                ContextMenuItem::Copy,
                ContextMenuItem::ToggleFavorite,
                ContextMenuItem::AddTag,
                ContextMenuItem::AddToCollection,
            ]
        );
    }

    #[test]
    fn test_mouse_right_click_menu_in_collections_view() {
        let mut state = state_with(&["a"]);
        state.view_mode = ViewMode::Collections;
        handle(&mut state, right(5, 11));
        assert_eq!(
            state.context_menu_items.last(),
            Some(&ContextMenuItem::RemoveFromCollection)
        );
    }

    /// A collections pane occupying rows 10..20 on the left.
    fn state_with_collections(names: &[&str]) -> AppState {
        let mut state = state_with(&["a", "b"]);
        state.view_mode = ViewMode::Collections;
        state.hit.collections_list = Rect::new(0, 10, 24, 10);
        state.hit.list = Rect::new(24, 10, 40, 10);
        state.collections = names
            .iter()
            .map(|n| crate::storage::collections::Collection {
                id: crate::storage::collections::hash_collection_name(n),
                name: n.to_string(),
            })
            .collect();
        state
    }

    #[test]
    fn test_mouse_right_click_on_a_collection_opens_the_collection_menu() {
        let mut state = state_with_collections(&["work", "personal"]);

        handle(&mut state, right(5, 12));

        assert!(state.context_menu_open);
        assert_eq!(state.active_pane, ActivePane::CollectionsList);
        assert_eq!(state.selected_collection_index, 1);
        assert_eq!(
            state.context_menu_items,
            vec![
                ContextMenuItem::OpenCollection,
                ContextMenuItem::RenameCollection,
                ContextMenuItem::DeleteCollection,
                ContextMenuItem::NewCollection,
            ]
        );
    }

    #[test]
    fn test_mouse_right_click_on_an_empty_collections_pane_offers_new() {
        let mut state = state_with_collections(&[]);

        handle(&mut state, right(5, 12));

        assert!(state.context_menu_open);
        assert_eq!(
            state.context_menu_items,
            vec![ContextMenuItem::NewCollection]
        );
    }

    #[test]
    fn test_mouse_collection_menu_delete_opens_the_confirm_popup() {
        let mut state = state_with_collections(&["work"]);
        handle(&mut state, right(5, 11));
        state.hit.context_menu = Rect::new(5, 11, 24, 6);

        // Third entry: Open, Rename, Delete.
        handle(&mut state, left(7, 14));

        assert!(!state.context_menu_open);
        assert_eq!(
            state.collection_input_mode,
            crate::app::CollectionInputMode::ConfirmDeleteCollection,
            "delete must go through the confirm popup, not remove anything itself"
        );
        assert_eq!(state.delete_confirm_text, "work");
        assert_eq!(state.collections.len(), 1, "nothing is deleted yet");
    }

    #[test]
    fn test_mouse_collection_menu_rename_prefills_the_name() {
        let mut state = state_with_collections(&["work"]);
        handle(&mut state, right(5, 11));
        state.hit.context_menu = Rect::new(5, 11, 24, 6);

        // Second entry: Rename.
        handle(&mut state, left(7, 13));

        assert_eq!(
            state.collection_input_mode,
            crate::app::CollectionInputMode::EditCollection
        );
        assert_eq!(state.collection_input_text, "work");
        assert!(state.editing_collection_id.is_some());
    }

    #[test]
    fn test_mouse_right_click_on_empty_list_opens_nothing() {
        let mut state = state_with(&[]);
        handle(&mut state, right(5, 11));
        assert!(!state.context_menu_open);
    }

    #[test]
    fn test_mouse_click_outside_menu_closes_it() {
        let mut state = state_with(&["a", "b"]);
        handle(&mut state, right(5, 11));
        state.hit.context_menu = Rect::new(5, 11, 20, 7);
        handle(&mut state, left(1, 1));
        assert!(!state.context_menu_open);
    }

    #[test]
    fn test_mouse_menu_run_entry_executes() {
        let mut state = state_with(&["ls -la", "git status"]);
        handle(&mut state, right(5, 12));
        state.hit.context_menu = Rect::new(5, 12, 20, 7);
        // Row 0 inside the menu, one below its top border, is "Run".
        assert_eq!(
            handle(&mut state, left(7, 13)),
            Action::Execute("git status".into())
        );
        assert!(!state.context_menu_open);
    }

    #[test]
    fn test_mouse_menu_favorite_entry_toggles() {
        let mut state = state_with(&["ls -la"]);
        handle(&mut state, right(5, 11));
        state.hit.context_menu = Rect::new(5, 11, 20, 7);
        // Third entry: Run, Copy, Favorite.
        handle(&mut state, left(7, 14));
        assert!(state.filtered[0].favorite);
        assert!(!state.context_menu_open);
    }

    /// A state whose details seam sits at columns 60-61 of a 100-wide content
    /// row, with the list above it.
    fn state_with_divider(texts: &[&str]) -> AppState {
        let mut state = state_with(texts);
        state.set_terminal_size(100, 30);
        state.hit.content = Rect::new(0, 10, 100, 10);
        state.hit.dividers[1] = Rect::new(60, 10, 2, 10);
        state
    }

    fn drag(x: u16, y: u16) -> MouseEvent {
        ev(MouseEventKind::Drag(MouseButton::Left), x, y)
    }

    #[test]
    fn test_mouse_press_on_a_divider_starts_a_drag_without_selecting() {
        let mut state = state_with_divider(&["a", "b", "c"]);
        state.select_index(1);

        handle(&mut state, left(60, 12));

        assert_eq!(state.dragging, Some(Divider::Details));
        assert_eq!(state.selected_index, 1, "grabbing a seam must not select");
    }

    #[test]
    fn test_mouse_drag_resizes_and_release_ends_it() {
        let mut state = state_with_divider(&["a"]);
        handle(&mut state, left(60, 12));

        handle(&mut state, drag(70, 12));
        assert_eq!(state.details_width, 30);
        handle(&mut state, drag(55, 12));
        assert_eq!(state.details_width, 45);

        handle(
            &mut state,
            ev(MouseEventKind::Up(MouseButton::Left), 55, 12),
        );
        assert_eq!(state.dragging, None);
    }

    #[test]
    fn test_mouse_drag_swallows_everything_else() {
        let mut state = state_with_divider(&["a", "b", "c"]);
        state.select_index(0);
        handle(&mut state, left(60, 12));

        // A wheel notch or a stray press mid-drag must not scroll or select.
        handle(&mut state, ev(MouseEventKind::ScrollDown, 5, 12));
        handle(&mut state, left(5, 13));
        assert_eq!(state.selected_index, 0);
        assert_eq!(state.dragging, Some(Divider::Details));
    }

    #[test]
    fn test_mouse_drag_past_the_minimum_collapses_details() {
        let mut state = state_with_divider(&["a"]);
        handle(&mut state, left(60, 12));
        handle(&mut state, drag(95, 12));

        assert!(!state.show_details);
        assert_eq!(state.dragging, None);
    }

    #[test]
    fn test_mouse_motion_without_a_drag_is_still_ignored() {
        let mut state = state_with_divider(&["a"]);
        let before = state.details_width;
        handle(&mut state, drag(70, 12));
        assert_eq!(state.details_width, before);
        assert_eq!(state.dragging, None);
    }

    #[test]
    fn test_mouse_integration_popup_swallows_clicks() {
        let mut state = state_with(&["a", "b"]);
        state.integration_popup_open = true;
        handle(&mut state, left(5, 12));
        assert_eq!(state.selected_index, 0);
        assert!(state.integration_popup_open);
    }

    #[test]
    fn test_mouse_click_outside_popup_dismisses_it() {
        let mut state = state_with(&["a"]);
        state.open_theme_popup();
        state.hit.popup = Rect::new(10, 10, 20, 8);
        handle(&mut state, left(1, 1));
        assert!(!state.theme_popup_open);
    }

    #[test]
    fn test_mouse_click_inside_popup_keeps_it_open() {
        let mut state = state_with(&["a"]);
        state.open_theme_popup();
        state.hit.popup = Rect::new(10, 10, 20, 8);
        handle(&mut state, left(12, 12));
        assert!(state.theme_popup_open);
    }

    #[test]
    fn test_mouse_wheel_moves_popup_selection() {
        let mut state = state_with(&["a"]);
        state.open_theme_popup();
        // The popup opens on whichever flavor is active; start from the top so
        // the assertions do not depend on the default theme.
        state.theme_popup_index = 0;
        state.hit.popup = Rect::new(10, 10, 20, 8);
        handle(&mut state, ev(MouseEventKind::ScrollDown, 12, 12));
        assert_eq!(state.theme_popup_index, 1);
        handle(&mut state, ev(MouseEventKind::ScrollUp, 12, 12));
        assert_eq!(state.theme_popup_index, 0);
    }
}
