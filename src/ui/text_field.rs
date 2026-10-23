//! A line of text being typed, with a caret: the library search box.

#[derive(Clone, Debug, Default)]
pub struct TextField {
    text: String,
    /// Where the caret is, in characters from the start.
    caret: usize,
}

impl TextField {
    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn caret(&self) -> usize {
        self.caret
    }

    /// Where the `index`th character starts, in bytes.
    fn byte(&self, index: usize) -> usize {
        self.text.char_indices().nth(index).map_or(self.text.len(), |(i, _)| i)
    }

    fn len(&self) -> usize {
        self.text.chars().count()
    }

    /// Types `typed` at the caret. Control characters (Tab, Enter...) are
    /// left out.
    pub fn insert(&mut self, typed: &str) {
        let typed: String = typed.chars().filter(|c| !c.is_control()).collect();
        let at = self.byte(self.caret);
        self.text.insert_str(at, &typed);
        self.caret += typed.chars().count();
    }

    pub fn backspace(&mut self) {
        if self.caret > 0 {
            self.caret -= 1;
            self.delete();
        }
    }

    /// Deletes the character after the caret.
    pub fn delete(&mut self) {
        if self.caret < self.len() {
            let at = self.byte(self.caret);
            self.text.remove(at);
        }
    }

    pub fn left(&mut self) {
        self.caret = self.caret.saturating_sub(1);
    }

    pub fn right(&mut self) {
        self.caret = (self.caret + 1).min(self.len());
    }

    pub fn home(&mut self) {
        self.caret = 0;
    }

    pub fn end(&mut self) {
        self.caret = self.len();
    }

    pub fn clear(&mut self) {
        self.text.clear();
        self.caret = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn typing_and_editing() {
        let mut field = TextField::default();
        field.insert("rado");
        field.left();
        field.insert("i");
        assert_eq!((field.text(), field.caret()), ("radio", 4));
        field.end();
        field.insert("héad\t");
        assert_eq!((field.text(), field.caret()), ("radiohéad", 9));
        field.backspace();
        field.home();
        field.delete();
        assert_eq!((field.text(), field.caret()), ("adiohéa", 0));
        field.left();
        field.backspace();
        assert_eq!(field.caret(), 0);
        field.end();
        field.right();
        field.delete();
        assert_eq!((field.text(), field.caret()), ("adiohéa", 7));
        field.clear();
        assert_eq!((field.text(), field.caret()), ("", 0));
    }
}
