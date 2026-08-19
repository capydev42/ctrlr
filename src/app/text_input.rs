//! A single-line text buffer with a real cursor.
//!
//! The five older input fields on [`AppState`](crate::app::AppState) —
//! `search_query`, `tag_input`, `collection_input_text`, `help_search_query`
//! and `import_export_file_path` — are plain `String`s edited with `push` and
//! `pop`, so they can only ever be appended to. That is fine for a search box
//! and useless for editing a command you are about to run. New inputs should
//! use this type; converting the old ones is a separate job, search bar first.
//!
//! `cursor` is a **byte** index and is always on a char boundary. History is
//! UTF-8 and routinely contains non-ASCII, so every move steps through
//! `char_indices` rather than doing byte arithmetic.

#[derive(Clone, Debug, Default, PartialEq)]
pub struct TextInput {
    value: String,
    cursor: usize,
}

impl TextInput {
    /// Starts with the cursor at the end, which is where someone recalling a
    /// command wants it.
    pub fn new(value: impl Into<String>) -> Self {
        let value = value.into();
        let cursor = value.len();
        Self { value, cursor }
    }

    pub fn value(&self) -> &str {
        &self.value
    }

    /// Byte offset of the cursor. Only the tests need it — the renderer works
    /// in columns via [`TextInput::cursor_col`] — but asserting the raw offset
    /// is what pins the char-boundary invariant.
    #[cfg(test)]
    pub fn cursor(&self) -> usize {
        self.cursor
    }

    #[cfg(test)]
    pub fn is_empty(&self) -> bool {
        self.value.is_empty()
    }

    /// Replaces the whole line, cursor to the end — what coming back from an
    /// external editor does.
    pub fn set_value(&mut self, value: impl Into<String>) {
        self.value = value.into();
        self.cursor = self.value.len();
    }

    pub fn clear(&mut self) {
        self.value.clear();
        self.cursor = 0;
    }

    pub fn insert(&mut self, c: char) {
        self.value.insert(self.cursor, c);
        self.cursor += c.len_utf8();
    }

    /// Deletes the character before the cursor.
    pub fn backspace(&mut self) {
        if self.cursor == 0 {
            return;
        }
        let prev = self.prev_boundary();
        self.value.remove(prev);
        self.cursor = prev;
    }

    /// Deletes the character under the cursor; a no-op at the end.
    pub fn delete(&mut self) {
        if self.cursor < self.value.len() {
            self.value.remove(self.cursor);
        }
    }

    pub fn left(&mut self) {
        self.cursor = self.prev_boundary();
    }

    pub fn right(&mut self) {
        self.cursor = self.next_boundary();
    }

    pub fn home(&mut self) {
        self.cursor = 0;
    }

    pub fn end(&mut self) {
        self.cursor = self.value.len();
    }

    /// Ctrl+U: drops everything before the cursor, matching what Ctrl+U does to
    /// the search box and the tag prompt.
    pub fn kill_to_start(&mut self) {
        self.value.drain(..self.cursor);
        self.cursor = 0;
    }

    /// Display columns before the cursor, for `frame.set_cursor_position`.
    /// Counts chars rather than measuring width: the renderer scrolls by the
    /// same unit, so the two agree even where they are both wrong about a wide
    /// glyph.
    pub fn cursor_col(&self) -> usize {
        self.value[..self.cursor].chars().count()
    }

    fn prev_boundary(&self) -> usize {
        self.value[..self.cursor]
            .char_indices()
            .next_back()
            .map(|(i, _)| i)
            .unwrap_or(0)
    }

    fn next_boundary(&self) -> usize {
        self.value[self.cursor..]
            .chars()
            .next()
            .map(|c| self.cursor + c.len_utf8())
            .unwrap_or(self.cursor)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_text_input_new_puts_cursor_at_end() {
        let input = TextInput::new("git status");
        assert_eq!(input.cursor(), 10);
        assert_eq!(input.value(), "git status");
    }

    #[test]
    fn test_text_input_inserts_at_the_cursor() {
        let mut input = TextInput::new("git tatus");
        input.home();
        for _ in 0..4 {
            input.right();
        }
        input.insert('s');
        assert_eq!(input.value(), "git status");
        assert_eq!(input.cursor(), 5);
    }

    #[test]
    fn test_text_input_backspace_at_start_is_a_no_op() {
        let mut input = TextInput::new("ls");
        input.home();
        input.backspace();
        assert_eq!(input.value(), "ls");
        assert_eq!(input.cursor(), 0);
    }

    #[test]
    fn test_text_input_delete_at_end_is_a_no_op() {
        let mut input = TextInput::new("ls");
        input.delete();
        assert_eq!(input.value(), "ls");
    }

    #[test]
    fn test_text_input_delete_removes_the_char_under_the_cursor() {
        let mut input = TextInput::new("lls");
        input.home();
        input.delete();
        assert_eq!(input.value(), "ls");
        assert_eq!(input.cursor(), 0);
    }

    #[test]
    fn test_text_input_kill_to_start_keeps_the_tail() {
        let mut input = TextInput::new("sudo rm -rf");
        input.home();
        for _ in 0..5 {
            input.right();
        }
        input.kill_to_start();
        assert_eq!(input.value(), "rm -rf");
        assert_eq!(input.cursor(), 0);
    }

    /// Byte arithmetic here would panic or corrupt the string; every move has
    /// to land on a char boundary.
    #[test]
    fn test_text_input_steps_over_multibyte_chars() {
        let mut input = TextInput::new("héllo");
        assert_eq!(input.cursor(), 6, "é is two bytes");
        input.home();
        input.right();
        input.right();
        assert_eq!(input.cursor(), 3, "past h and é");
        input.insert('x');
        assert_eq!(input.value(), "héxllo");
        input.backspace();
        assert_eq!(input.value(), "héllo");
        input.backspace();
        assert_eq!(input.value(), "hllo");
    }

    #[test]
    fn test_text_input_cursor_col_counts_chars_not_bytes() {
        let mut input = TextInput::new("héllo");
        assert_eq!(input.cursor_col(), 5);
        input.home();
        input.right();
        input.right();
        assert_eq!(input.cursor_col(), 2);
    }

    #[test]
    fn test_text_input_right_at_end_and_left_at_start_are_no_ops() {
        let mut input = TextInput::new("ls");
        input.right();
        assert_eq!(input.cursor(), 2);
        input.home();
        input.left();
        assert_eq!(input.cursor(), 0);
    }
}
