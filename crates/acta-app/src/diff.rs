//! Line changes between two revisions (docs/design/components-knowledge.md
//! 5, History: "Changes shows the raw Markdown with a 16 gutter"), by the
//! longest common subsequence of lines.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Change {
    Same,
    Added,
    Removed,
}

/// Above this many lines on either side the table would be too large to
/// compute on a keystroke; the whole text is then shown as replaced.
const MAX_LINES: usize = 4000;

pub fn lines<'a>(old: &'a str, new: &'a str) -> Vec<(Change, &'a str)> {
    let a: Vec<&str> = old.lines().collect();
    let b: Vec<&str> = new.lines().collect();
    if a.len() > MAX_LINES || b.len() > MAX_LINES {
        let mut out: Vec<(Change, &str)> = a.iter().map(|l| (Change::Removed, *l)).collect();
        out.extend(b.iter().map(|l| (Change::Added, *l)));
        return out;
    }
    // lcs[i][j]: common length of a[i..] and b[j..].
    let mut lcs = vec![vec![0u32; b.len() + 1]; a.len() + 1];
    for i in (0..a.len()).rev() {
        for j in (0..b.len()).rev() {
            lcs[i][j] = if a[i] == b[j] {
                lcs[i + 1][j + 1] + 1
            } else {
                lcs[i + 1][j].max(lcs[i][j + 1])
            };
        }
    }
    let (mut i, mut j) = (0, 0);
    let mut out = Vec::with_capacity(a.len().max(b.len()));
    while i < a.len() && j < b.len() {
        if a[i] == b[j] {
            out.push((Change::Same, a[i]));
            i += 1;
            j += 1;
        } else if lcs[i + 1][j] >= lcs[i][j + 1] {
            out.push((Change::Removed, a[i]));
            i += 1;
        } else {
            out.push((Change::Added, b[j]));
            j += 1;
        }
    }
    out.extend(a[i..].iter().map(|l| (Change::Removed, *l)));
    out.extend(b[j..].iter().map(|l| (Change::Added, *l)));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn changed_lines_are_marked_and_the_rest_kept() {
        let old = "# Goals\nReflow on resize.\nKeep scrollback.";
        let new = "# Goals\nReflow on every resize.\nKeep scrollback.\nNo flicker.";
        let out = lines(old, new);
        assert_eq!(
            out,
            [
                (Change::Same, "# Goals"),
                (Change::Removed, "Reflow on resize."),
                (Change::Added, "Reflow on every resize."),
                (Change::Same, "Keep scrollback."),
                (Change::Added, "No flicker."),
            ]
        );
    }
}
