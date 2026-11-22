//! Text forms of the domain enums in the database and in exports. They are
//! the serde names, so the database, the export file and agent output agree.

use crate::model::{
    Author, ChangeKind, CommitMode, EntryKind, EventKind, KbKind, Level, QuestionStatus, Status,
};
use rusqlite::types::{FromSql, FromSqlError, FromSqlResult, ToSql, ToSqlOutput, ValueRef};

macro_rules! text_enum {
    ($ty:ty { $($variant:ident => $text:literal),+ $(,)? }) => {
        impl $ty {
            pub fn as_str(self) -> &'static str {
                match self { $(<$ty>::$variant => $text),+ }
            }

            pub fn parse(text: &str) -> Option<Self> {
                match text { $($text => Some(<$ty>::$variant),)+ _ => None }
            }
        }

        impl ToSql for $ty {
            fn to_sql(&self) -> rusqlite::Result<ToSqlOutput<'_>> {
                Ok(ToSqlOutput::from(self.as_str()))
            }
        }

        impl FromSql for $ty {
            fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
                let text = value.as_str()?;
                <$ty>::parse(text).ok_or_else(|| FromSqlError::Other(
                    format!("unknown {} `{text}`", stringify!($ty)).into(),
                ))
            }
        }
    };
}

text_enum!(Author { Human => "human", Claude => "claude" });
text_enum!(Status {
    Draft => "draft",
    Planned => "planned",
    Active => "active",
    Blocked => "blocked",
    Done => "done",
    Dropped => "dropped",
});
text_enum!(EntryKind {
    Progress => "progress",
    Decision => "decision",
    Discussion => "discussion",
    System => "system",
});
text_enum!(EventKind {
    Created => "created",
    Status => "status",
    Moved => "moved",
    State => "state",
    Restored => "restored",
    Edited => "edited",
    Due => "due",
    Report => "report",
    Reported => "reported",
});
text_enum!(QuestionStatus { Open => "open", Answered => "answered", Dropped => "dropped" });
text_enum!(CommitMode { Manual => "manual", Commit => "commit", Push => "push" });
text_enum!(KbKind { Document => "document", Memory => "memory" });
text_enum!(Level { Project => "project", Epic => "epic", Topic => "topic" });
text_enum!(ChangeKind {
    Created => "created",
    CreatedFromFile => "created_from_file",
    Edited => "edited",
    ReplacedSection => "replaced_section",
    AppendedSection => "appended_section",
    Moved => "moved",
    Restored => "restored",
});

/// A due day is stored as its ISO text (schema V4 checks the shape).
impl ToSql for crate::date::Date {
    fn to_sql(&self) -> rusqlite::Result<ToSqlOutput<'_>> {
        Ok(ToSqlOutput::from(self.to_string()))
    }
}

impl FromSql for crate::date::Date {
    fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
        let text = value.as_str()?;
        crate::date::Date::parse_iso(text)
            .ok_or_else(|| FromSqlError::Other(format!("not a due date `{text}`").into()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The database text and the serde (export) text must never drift apart.
    #[test]
    fn text_forms_match_serde() {
        fn check<T: serde::Serialize>(value: T, text: &str) {
            assert_eq!(serde_json::to_value(value).unwrap(), text);
        }
        for s in Status::ALL {
            check(s, s.as_str());
            assert_eq!(Status::parse(s.as_str()), Some(s));
        }
        check(Author::Claude, Author::Claude.as_str());
        check(EntryKind::System, EntryKind::System.as_str());
        check(EventKind::Restored, EventKind::Restored.as_str());
        check(EventKind::Edited, EventKind::Edited.as_str());
        check(EventKind::Due, EventKind::Due.as_str());
        check(EventKind::Report, EventKind::Report.as_str());
        check(EventKind::Reported, EventKind::Reported.as_str());
        for q in [
            QuestionStatus::Open,
            QuestionStatus::Answered,
            QuestionStatus::Dropped,
        ] {
            check(q, q.as_str());
        }
        for m in [CommitMode::Manual, CommitMode::Commit, CommitMode::Push] {
            check(m, m.as_str());
        }
        check(KbKind::Memory, KbKind::Memory.as_str());
        check(Level::Epic, Level::Epic.as_str());
        check(
            ChangeKind::CreatedFromFile,
            ChangeKind::CreatedFromFile.as_str(),
        );
        check(
            ChangeKind::ReplacedSection,
            ChangeKind::ReplacedSection.as_str(),
        );
        assert_eq!(Status::parse("Doing"), None);
    }
}
