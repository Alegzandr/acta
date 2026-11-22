//! Where dictated text goes (components-voice.md 5, "Where the text goes"):
//! at an anchor, the caret where the take started, moved by every edit
//! before it so typing elsewhere never splits a sentence. A space goes
//! before a phrase that follows anything but a space or a line start;
//! nothing else is added or changed. The caret follows the inserted text
//! only when it was at the anchor.
//!
//! Positions are byte offsets into the editor's UTF-8 text, always on a
//! character boundary.

/// The anchor of a take in one editor.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Anchor {
    pos: usize,
}

/// One phrase written into the text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Insertion {
    /// Byte offset where `text` goes.
    pub at: usize,
    /// What to insert: the phrase, with a leading space when one is needed.
    pub text: String,
    /// The caret after the insertion, when it moves.
    pub caret: Option<usize>,
}

impl Anchor {
    /// The anchor at the caret where the take starts.
    pub fn new(caret: usize) -> Self {
        Anchor { pos: caret }
    }

    pub fn pos(&self) -> usize {
        self.pos
    }

    /// Follows an edit of the text: `removed` bytes at `at` replaced by
    /// `inserted` bytes. An edit wholly before the anchor shifts it; one
    /// after leaves it; a deletion that covers it brings it to the start of
    /// the edit. Typing at the anchor itself goes before the anchor, as it
    /// would before a caret, so the next phrase comes after what was typed.
    pub fn edited(&mut self, at: usize, removed: usize, inserted: usize) {
        let end = at + removed;
        if end <= self.pos {
            if at == self.pos && removed == 0 {
                self.pos += inserted;
            } else {
                self.pos = self.pos - removed + inserted;
            }
        } else if at < self.pos {
            // The deletion covers the anchor.
            self.pos = at + inserted;
        }
    }

    /// Writes `phrase` (as the engine gave it) at the anchor of `text`.
    /// `caret` is the editor's caret now. Returns `None` for a phrase with
    /// no text. The anchor moves past the inserted text; the caller applies
    /// the insertion (as one undo step with the rest of the take).
    pub fn insert(&mut self, text: &str, phrase: &str, caret: usize) -> Option<Insertion> {
        let phrase = phrase.trim();
        if phrase.is_empty() {
            return None;
        }
        let at = self.pos.min(text.len());
        let needs_space = text[..at]
            .chars()
            .next_back()
            .is_some_and(|c| !c.is_whitespace());
        let mut out = String::with_capacity(phrase.len() + 1);
        if needs_space {
            out.push(' ');
        }
        out.push_str(phrase);
        let follows = caret == at;
        self.pos = at + out.len();
        Some(Insertion {
            at,
            caret: follows.then_some(self.pos),
            text: out,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn apply(text: &mut String, ins: &Insertion) {
        text.insert_str(ins.at, &ins.text);
    }

    /// Text with `|` marking the caret; returns the text and its offset.
    fn at_bar(marked: &str) -> (String, usize) {
        let i = marked.find('|').unwrap();
        (marked.replacen('|', "", 1), i)
    }

    #[test]
    fn the_table_of_texts_and_anchors() {
        // (text with the caret, phrase, expected text, caret follows)
        let rows = [
            ("|", " Hello Alexandre.", "Hello Alexandre.", true),
            ("|Existing.", "Hello.", "Hello.Existing.", true),
            ("End of line.|", " Next.", "End of line. Next.", true),
            ("End of line. |", "Next.", "End of line. Next.", true),
            ("Line one\n|", "Line two.", "Line one\nLine two.", true),
            ("Line one\n|rest", "Two.", "Line one\nTwo.rest", true),
            // Mid-word: a space before, nothing after.
            ("Alex|andre", "said", "Alex saidandre", true),
            ("Tab\t|", "x", "Tab\tx", true),
            ("Café|", "noir.", "Café noir.", true),
            ("|", "   ", "", true),
        ];
        for (marked, phrase, want, _) in rows {
            let (mut text, caret) = at_bar(marked);
            let mut anchor = Anchor::new(caret);
            if let Some(ins) = anchor.insert(&text, phrase, caret) {
                apply(&mut text, &ins);
                assert_eq!(ins.caret, Some(anchor.pos()), "{marked:?}");
            }
            assert_eq!(text, want, "{marked:?} + {phrase:?}");
        }
    }

    #[test]
    fn phrases_follow_one_another() {
        let mut text = String::from("Notes: ");
        let mut anchor = Anchor::new(text.len());
        for phrase in [" First phrase.", " Second phrase.", "Third."] {
            let caret = anchor.pos();
            let ins = anchor.insert(&text, phrase, caret).unwrap();
            apply(&mut text, &ins);
        }
        assert_eq!(text, "Notes: First phrase. Second phrase. Third.");
    }

    #[test]
    fn edits_before_the_anchor_move_it_and_edits_after_do_not() {
        let (mut text, caret) = at_bar("Start. | End.");
        let mut anchor = Anchor::new(caret);
        // Typed before the anchor: "Alexandre " at 0.
        text.insert_str(0, "Alexandre ");
        anchor.edited(0, 0, 10);
        // Typed after it.
        let end = text.len();
        text.push_str(" More.");
        anchor.edited(end, 0, 6);
        // The caret is elsewhere now: it does not follow.
        let ins = anchor.insert(&text, "Spoken.", 0).unwrap();
        assert_eq!(ins.caret, None);
        apply(&mut text, &ins);
        assert_eq!(text, "Alexandre Start. Spoken. End. More.");
    }

    #[test]
    fn a_deletion_before_or_over_the_anchor() {
        let (mut text, caret) = at_bar("abc def|ghi");
        let mut anchor = Anchor::new(caret);
        // Delete "abc " before it.
        text.replace_range(0..4, "");
        anchor.edited(0, 4, 0);
        assert_eq!(anchor.pos(), 3);
        // Select "efg" across it and type "X".
        text.replace_range(1..4, "X");
        anchor.edited(1, 3, 1);
        assert_eq!(anchor.pos(), 2);
        let ins = anchor.insert(&text, "said", 2).unwrap();
        apply(&mut text, &ins);
        assert_eq!(text, "dX saidhi");
        assert_eq!(ins.caret, Some(7));
    }

    #[test]
    fn typing_at_the_anchor_goes_before_the_next_phrase() {
        let mut text = String::from("One.");
        let mut anchor = Anchor::new(4);
        text.push_str(" typed");
        anchor.edited(4, 0, 6);
        let caret = text.len();
        let ins = anchor.insert(&text, "Two.", caret).unwrap();
        apply(&mut text, &ins);
        assert_eq!(text, "One. typed Two.");
        assert_eq!(ins.caret, Some(text.len()));
    }

    #[test]
    fn an_edit_after_the_anchor_but_touching_it_leaves_it() {
        let mut anchor = Anchor::new(5);
        anchor.edited(5, 3, 0);
        assert_eq!(anchor.pos(), 5);
        anchor.edited(6, 0, 4);
        assert_eq!(anchor.pos(), 5);
    }

    #[test]
    fn an_anchor_past_a_shortened_text_is_clamped() {
        let text = "ab";
        let mut anchor = Anchor::new(10);
        let ins = anchor.insert(text, "c", 0).unwrap();
        assert_eq!(ins.at, 2);
        assert_eq!(ins.text, " c");
    }
}
