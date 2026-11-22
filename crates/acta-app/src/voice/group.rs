//! One take, one undo step (components-voice.md 5). A take writes its text
//! phrase by phrase, and the editor (gpui-base's input) records each
//! insertion as an undo transaction of its own; its undo manager is
//! private to that crate, so the phrases cannot be merged there. Acta
//! groups them at its own level instead: it remembers, per editor, how
//! many phrases the last take wrote and the text before and after it.
//! `Mod+Z` pressed while the editor still holds exactly the text the take
//! left takes back all its phrases at once (the key's own undo plus one
//! more per extra phrase, sent to the editor), and `Mod+Shift+Z` right
//! after puts them all back the same way. Anything else (typing during or
//! after the take, an undo past it) leaves the editor's own undo alone.

/// The last take of an editor.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TakeGroup {
    phrases: usize,
    before: String,
    after: String,
    /// No typing happened in the editor while the take wrote: its phrases
    /// are the newest transactions, back to back.
    clean: bool,
    /// The take was just taken back as one step.
    undone: bool,
}

impl TakeGroup {
    /// A take starts in an editor holding `value`.
    pub fn start(value: &str) -> Self {
        TakeGroup {
            phrases: 0,
            before: value.to_owned(),
            after: value.to_owned(),
            clean: true,
            undone: false,
        }
    }

    /// A phrase was written; the editor now holds `value`.
    pub fn phrase(&mut self, value: &str) {
        self.phrases += 1;
        self.after = value.to_owned();
        self.undone = false;
    }

    /// The person edited the editor while the take was writing.
    pub fn typed(&mut self) {
        self.clean = false;
    }

    /// `Mod+Z` with the editor holding `value`: how many undo steps to
    /// send after the key's own, so the whole take goes back at once.
    pub fn undo(&mut self, value: &str) -> usize {
        if !self.clean || self.undone || self.phrases < 2 || value != self.after {
            return 0;
        }
        self.undone = true;
        self.phrases - 1
    }

    /// `Mod+Shift+Z` with the editor holding `value`: how many redo steps
    /// to send after the key's own, so the whole take comes back.
    pub fn redo(&mut self, value: &str) -> usize {
        if !self.clean || !self.undone || value != self.before {
            return 0;
        }
        self.undone = false;
        self.phrases - 1
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn take(phrases: &[&str]) -> TakeGroup {
        let mut g = TakeGroup::start("Note: ");
        let mut text = String::from("Note: ");
        for p in phrases {
            text.push_str(p);
            g.phrase(&text);
        }
        g
    }

    #[test]
    fn a_take_of_three_phrases_goes_back_and_comes_back_as_one_step() {
        let mut g = take(&["Alexandre called. ", "The export works. ", "Ship it."]);
        let after = "Note: Alexandre called. The export works. Ship it.";
        // The key's own undo, plus two more.
        assert_eq!(g.undo(after), 2);
        // Once back, a second undo is the editor's own (older history).
        assert_eq!(g.undo("Note: "), 0);
        assert_eq!(g.redo("Note: "), 2);
        assert_eq!(g.undo(after), 2);
    }

    #[test]
    fn one_phrase_or_typing_leaves_the_editor_undo_alone() {
        let mut one = take(&["Hello."]);
        assert_eq!(one.undo("Note: Hello."), 0);
        // Typed after the take: the text differs, so the key undoes the
        // typing only.
        let mut later = take(&["One. ", "Two."]);
        assert_eq!(later.undo("Note: One. Two.!"), 0);
        // Typed while the take wrote: the phrases are not back to back.
        let mut during = take(&["One. ", "Two."]);
        during.typed();
        assert_eq!(during.undo("Note: One. Two."), 0);
        // A redo without the grouped undo first is the editor's own.
        let mut fresh = take(&["One. ", "Two."]);
        assert_eq!(fresh.redo("Note: "), 0);
    }
}
