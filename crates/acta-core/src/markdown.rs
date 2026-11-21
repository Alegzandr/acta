//! Sections of a Markdown body (ADR 0005, 0007): the outline an agent reads
//! instead of a long document, and section-level edits so it never re-sends a
//! whole document to change a paragraph.
//!
//! Headings are ATX headings (`#` to `######`) outside fenced code blocks.
//! Sections are numbered by outline position (`1`, `2`, `2.1`), which settles
//! duplicate headings; a number is valid for the revision it was read from.
//! A section runs from its heading to the next heading of the same or a
//! higher level, so it includes its subsections.

/// One heading of the outline.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Section {
    /// Outline number: `1`, `3.1`.
    pub number: String,
    /// Heading level, 1 to 6.
    pub level: usize,
    pub title: String,
    /// Line index of the heading.
    pub start: usize,
    /// Line index one past the last line of the section.
    pub end: usize,
    /// Bytes of the section, heading included.
    pub bytes: usize,
}

fn heading(line: &str) -> Option<(usize, &str)> {
    let t = line.trim_start_matches(' ');
    if line.len() - t.len() > 3 {
        return None;
    }
    let level = t.bytes().take_while(|b| *b == b'#').count();
    if !(1..=6).contains(&level) {
        return None;
    }
    let rest = &t[level..];
    if !(rest.is_empty() || rest.starts_with([' ', '\t'])) {
        return None;
    }
    let title = rest.trim().trim_end_matches('#').trim_end();
    Some((level, title))
}

fn fence(line: &str) -> Option<&str> {
    let t = line.trim_start();
    if t.starts_with("```") {
        Some("```")
    } else if t.starts_with("~~~") {
        Some("~~~")
    } else {
        None
    }
}

/// The headings of a body with their numbers and extents.
pub fn outline(body: &str) -> Vec<Section> {
    let lines: Vec<&str> = body.lines().collect();
    let mut found: Vec<(usize, usize, String)> = Vec::new();
    let mut in_fence: Option<&str> = None;
    for (i, line) in lines.iter().enumerate() {
        if let Some(f) = in_fence {
            if line.trim_start().starts_with(f) {
                in_fence = None;
            }
            continue;
        }
        if let Some(f) = fence(line) {
            in_fence = Some(f);
            continue;
        }
        if let Some((level, title)) = heading(line) {
            found.push((i, level, title.to_owned()));
        }
    }
    let mut stack: Vec<(usize, usize)> = Vec::new();
    let mut out = Vec::with_capacity(found.len());
    for (n, (start, level, title)) in found.iter().enumerate() {
        // Popping deeper headings frees their depth slot; a heading that
        // takes the slot continues its count, so `# A / ### C / ## D` gives
        // 1, 1.1, 1.2 and `## A / # B` gives 1, 2: numbers never repeat.
        let mut freed = None;
        while stack.last().is_some_and(|(l, _)| *l > *level) {
            freed = stack.pop().map(|(_, count)| count);
        }
        match stack.last_mut() {
            Some((l, count)) if *l == *level => *count += 1,
            _ => stack.push((*level, freed.map_or(1, |c| c + 1))),
        }
        let number = stack
            .iter()
            .map(|(_, c)| c.to_string())
            .collect::<Vec<_>>()
            .join(".");
        let end = found[n + 1..]
            .iter()
            .find(|(_, l, _)| l <= level)
            .map_or(lines.len(), |(s, _, _)| *s);
        let bytes = lines[*start..end].iter().map(|l| l.len() + 1).sum();
        out.push(Section {
            number,
            level: *level,
            title: title.clone(),
            start: *start,
            end,
            bytes,
        });
    }
    out
}

/// The text of one section, heading included.
pub fn section_text(body: &str, number: &str) -> Option<String> {
    let s = outline(body).into_iter().find(|s| s.number == number)?;
    Some(join(&body.lines().collect::<Vec<_>>()[s.start..s.end]))
}

/// Replaces the content of a section (everything under its heading, its
/// subsections included) and optionally renames its heading. Returns the new
/// body and the section's title, or `None` if there is no such section.
pub fn replace_section(
    body: &str,
    number: &str,
    new_heading: Option<&str>,
    text: &str,
) -> Option<(String, String)> {
    let s = outline(body).into_iter().find(|s| s.number == number)?;
    let lines: Vec<&str> = body.lines().collect();
    let title = new_heading.map_or(s.title.clone(), |h| h.trim().to_owned());
    let mut out: Vec<String> = lines[..s.start].iter().map(|l| l.to_string()).collect();
    out.push(format!("{} {}", "#".repeat(s.level), title));
    out.push(String::new());
    let text = text.trim_matches('\n');
    if !text.is_empty() {
        out.extend(text.lines().map(str::to_owned));
        out.push(String::new());
    }
    out.extend(lines[s.end..].iter().map(|l| l.to_string()));
    Some((join_owned(&out), title))
}

/// Appends a section at the end of the body, at level 2 unless the body's
/// headings start deeper.
pub fn append_section(body: &str, heading: &str, text: &str) -> String {
    let level = outline(body)
        .iter()
        .map(|s| s.level)
        .min()
        .unwrap_or(2)
        .max(1);
    let mut out = body.trim_end_matches('\n').to_owned();
    if !out.is_empty() {
        out.push_str("\n\n");
    }
    out.push_str(&format!("{} {}\n", "#".repeat(level), heading.trim()));
    let text = text.trim_matches('\n');
    if !text.is_empty() {
        out.push('\n');
        out.push_str(text);
        out.push('\n');
    }
    out
}

fn join(lines: &[&str]) -> String {
    let mut s = lines.join("\n");
    s.push('\n');
    s
}

fn join_owned(lines: &[String]) -> String {
    let mut end = lines.len();
    while end > 0 && lines[end - 1].is_empty() {
        end -= 1;
    }
    let mut s = lines[..end].join("\n");
    s.push('\n');
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    const DOC: &str = "Intro line\n\n# Overview\nText\n\n# Crates\n## acta-core\nCore\n## acta-app\n```\n# not a heading\n```\n# Crates\nDuplicate\n";

    #[test]
    fn outline_numbers_nesting_and_fences() {
        let o = outline(DOC);
        let numbers: Vec<_> = o
            .iter()
            .map(|s| (s.number.as_str(), s.title.as_str()))
            .collect();
        assert_eq!(
            numbers,
            [
                ("1", "Overview"),
                ("2", "Crates"),
                ("2.1", "acta-core"),
                ("2.2", "acta-app"),
                ("3", "Crates"),
            ]
        );
        // Section 2 ends at the second "Crates", and includes its subsections.
        assert_eq!(o[1].end, o[4].start);
        assert_eq!(o[3].end, o[4].start);
        assert!(
            section_text(DOC, "2.2")
                .unwrap()
                .contains("# not a heading")
        );
        assert_eq!(section_text(DOC, "3").unwrap(), "# Crates\nDuplicate\n");
        assert!(outline("no headings at all").is_empty());
        assert!(outline("#hashtag\n    # indented code\n").is_empty());
    }

    #[test]
    fn numbers_never_repeat_when_levels_skip() {
        let numbers =
            |body: &str| -> Vec<String> { outline(body).into_iter().map(|s| s.number).collect() };
        assert_eq!(
            numbers(
                "# A
### C
## D
"
            ),
            ["1", "1.1", "1.2"]
        );
        assert_eq!(
            numbers(
                "## A
# B
"
            ),
            ["1", "2"]
        );
        assert_eq!(
            numbers(
                "# A
## B
#### C
### D
# E
"
            ),
            ["1", "1.1", "1.1.1", "1.1.2", "2"]
        );
        for body in [
            "# A
### C
## D
",
            "## A
# B
",
            "### a
## b
# c
## d
### e
",
        ] {
            let mut all = numbers(body);
            let n = all.len();
            all.sort();
            all.dedup();
            assert_eq!(all.len(), n, "{body:?}");
        }
    }

    #[test]
    fn replace_keeps_the_rest() {
        let (body, title) = replace_section(DOC, "2", None, "Only one crate now.").unwrap();
        assert_eq!(title, "Crates");
        assert!(body.starts_with("Intro line\n\n# Overview\nText\n\n# Crates\n\nOnly one crate now.\n\n# Crates\nDuplicate\n"));
        let (body, title) = replace_section(DOC, "1", Some("Summary"), "").unwrap();
        assert_eq!(title, "Summary");
        assert!(body.contains("# Summary\n\n# Crates"));
        assert!(replace_section(DOC, "9", None, "x").is_none());
    }

    #[test]
    fn append_matches_the_top_level() {
        assert_eq!(append_section("", "Notes", "Hi"), "## Notes\n\nHi\n");
        let body = append_section(DOC, "Open questions", "None.");
        assert!(body.ends_with("Duplicate\n\n# Open questions\n\nNone.\n"));
        assert_eq!(outline(&body).last().unwrap().number, "4");
    }
}
