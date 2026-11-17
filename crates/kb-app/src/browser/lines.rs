//! Page turns like a book reader's: no overlap, no cut lines, and the text lines of the new screen on the
//! rows the old lines used.
//!
//! On e-ink a turn changes only the pixels that differ. When the lines of the next screen sit on the rows of
//! the current one, the turn is a calm partial update; when every line lands a few pixels off (a turn that
//! scrolls a fixed share of the screen), nearly every pixel of every line changes and the turn reads as a
//! flash. Measured on the Oasis 3 with the same page in both ways.
//!
//! Forward, the next screen starts with the line after the last whole line, and nothing below the old screen
//! is skipped. Within the room that leaves (the gap between the two lines), the scroll position is the one
//! that puts the most new lines on old line rows; on a tie, the one that puts the first new line where the
//! first old line was. A turn back mirrors it: the line before the first whole line becomes the last.
//! Paragraph gaps that are not whole lines shift the lines after them, so such pages align as far as the
//! text allows.

use kb_engine::TextLine;

/// Glyphs may reach a little past their typographic box (stacked marks); a line counts as whole with this
/// much of it off screen.
const SLACK: f64 = 2.0;
/// Two rows closer than this are the same row.
const SAME_ROW: f64 = 1.5;

/// The screen around a turn: the scroll position (device pixels) and the visible rows of the page before
/// and after the turn (the bars hide on a forward turn and show on a turn back).
pub(super) struct Turn {
    pub scroll: f64,
    pub max_scroll: f64,
    pub top: f64,
    pub bottom: f64,
    pub next_top: f64,
    pub next_bottom: f64,
}

impl Turn {
    fn view(&self) -> f64 {
        self.bottom - self.top
    }
    fn next_view(&self) -> f64 {
        self.next_bottom - self.next_top
    }
}

/// The lines wholly inside the document range [from, to).
fn whole(lines: &[TextLine], from: f64, to: f64) -> impl Iterator<Item = &TextLine> {
    lines.iter().filter(move |l| l.top >= from - SLACK && l.bottom <= to + SLACK)
}

/// The new scroll position for a turn (`dir` > 0 forward), or `None` when the page has no text to align to
/// (the caller turns by a screen without overlap).
pub(super) fn turn_to(lines: &[TextLine], t: &Turn, dir: f64) -> Option<f64> {
    let s = t.scroll;
    let old: Vec<&TextLine> = whole(lines, s, s + t.view()).collect();
    let (first, last) = (*old.first()?, *old.last()?);
    // The old lines as on screen.
    let old_screen: Vec<TextLine> = old.iter().map(|l| shifted(l, t.top - s)).collect();
    let (lo, hi, ideal) = if dir > 0.0 {
        // The line after the last whole line must be whole at the top, the last whole line must not come back,
        // and the new screen must not start below the old one's end (that would skip content).
        let next = lines.iter().find(|l| l.baseline > last.baseline + SAME_ROW)?;
        let lo = last.bottom.max(last.baseline + 1.0);
        let hi = next.top.min(s + t.view());
        // The first new line where the first old line was.
        let ideal = next.baseline + t.next_top - (first.baseline - s + t.top);
        (lo, hi, ideal)
    } else {
        if s <= 0.0 {
            return None;
        }
        let prev = lines.iter().rev().find(|l| l.baseline < first.baseline - SAME_ROW)?;
        // The line before the first whole line must be whole at the bottom, the first whole line must not come
        // back, and the new screen must reach the old one's start.
        let lo = prev.bottom.max(s) - t.next_view();
        let hi = (first.top - t.next_view()).min(first.baseline - 1.0 - t.next_view());
        let ideal = prev.baseline + t.next_top - (last.baseline - s + t.top);
        (lo.max(0.0), hi, ideal)
    };
    let (start, end) = (lo.ceil(), hi.floor().min(t.max_scroll));
    if end < start {
        return None;
    }
    // Fewest new lines that land across an old one; then most on old rows; then nearest the ideal.
    let mut best: Option<((usize, usize, f64), f64)> = None;
    let mut c = start;
    while c <= end {
        let new: Vec<TextLine> = whole(lines, c, c + t.next_view()).map(|l| shifted(l, t.next_top - c)).collect();
        let (across, on_rows) = fit(&new, &old_screen);
        let key = (across, usize::MAX - on_rows, (c - ideal).abs());
        if best.map_or(true, |(k, _)| key < k) {
            best = Some((key, c));
        }
        c += 1.0;
    }
    best.map(|(_, c)| c)
}

fn shifted(l: &TextLine, by: f64) -> TextLine {
    TextLine { top: l.top + by, baseline: l.baseline + by, bottom: l.bottom + by }
}

/// How the new lines meet the old ones on screen: (lines that land across an old line, lines on an old line's
/// row). A new line on an old row, or on rows the old lines left blank, changes cleanly; one that overlaps an
/// old line a few rows off changes nearly all of both lines' pixels.
fn fit(new: &[TextLine], old: &[TextLine]) -> (usize, usize) {
    let mut across = 0;
    let mut on_rows = 0;
    for n in new {
        if old.iter().any(|o| (o.baseline - n.baseline).abs() <= SAME_ROW) {
            on_rows += 1;
        } else if old.iter().any(|o| n.top < o.bottom && n.bottom > o.top) {
            across += 1;
        }
    }
    (across, on_rows)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Lines of a page: each a glyph box from 40 above its baseline to 12 below.
    fn line(baseline: f64) -> TextLine {
        TextLine { top: baseline - 40.0, baseline, bottom: baseline + 12.0 }
    }

    /// Paragraphs of `n` lines every `pitch` rows, with `gap` extra rows between paragraphs.
    fn page(first: f64, pitch: f64, per_para: usize, gap: f64, total: usize) -> Vec<TextLine> {
        let mut y = first;
        let mut out = Vec::new();
        for i in 0..total {
            out.push(line(y));
            y += pitch;
            if (i + 1) % per_para == 0 {
                y += gap;
            }
        }
        out
    }

    fn screen(scroll: f64) -> Turn {
        Turn { scroll, max_scroll: 1e9, top: 0.0, bottom: 1000.0, next_top: 0.0, next_bottom: 1000.0 }
    }

    fn rows(lines: &[TextLine], scroll: f64, t: &Turn) -> Vec<f64> {
        whole(lines, scroll, scroll + t.next_view()).map(|l| l.baseline - scroll + t.next_top).collect()
    }

    #[test]
    fn plain_text_keeps_every_line_on_its_row() {
        let lines = page(63.0, 76.0, 1000, 0.0, 100);
        let t = screen(0.0);
        let next = turn_to(&lines, &t, 1.0).unwrap();
        // The last whole line is 975 (63 + 12 * 76); the next one, 1051, takes the first row (63).
        assert_eq!(next, 1051.0 - 63.0);
        assert_eq!(rows(&lines, next, &t), rows(&lines, 0.0, &t));
        // And back again.
        let t2 = screen(next);
        assert_eq!(turn_to(&lines, &t2, -1.0), Some(0.0));
    }

    #[test]
    fn nothing_is_skipped_or_repeated() {
        let lines = page(63.0, 76.0, 5, 8.0, 200);
        let mut s = 0.0;
        let mut seen = Vec::new();
        for _ in 0..10 {
            let t = screen(s);
            seen.extend(whole(&lines, s, s + 1000.0).map(|l| l.baseline));
            s = turn_to(&lines, &t, 1.0).unwrap();
        }
        let mut sorted = seen.clone();
        sorted.dedup();
        assert_eq!(sorted.len(), seen.len(), "a line was shown twice");
        let expected: Vec<f64> = lines.iter().map(|l| l.baseline).take(seen.len()).collect();
        assert_eq!(seen, expected, "a line was skipped");
    }

    /// How many of the new lines of `turns` forward turns change cleanly (on an old row or on blank rows).
    fn aligned_share(lines: &[TextLine], turns: usize) -> (usize, usize) {
        let (mut on, mut all) = (0, 0);
        let mut s = 0.0;
        for _ in 0..turns {
            let t = screen(s);
            let old: Vec<TextLine> = whole(lines, s, s + 1000.0).map(|l| shifted(l, -s)).collect();
            let next = turn_to(lines, &t, 1.0).unwrap();
            let new: Vec<TextLine> = whole(lines, next, next + 1000.0).map(|l| shifted(l, -next)).collect();
            let (across, _) = fit(&new, &old);
            on += new.len() - across;
            all += new.len();
            s = next;
        }
        (on, all)
    }

    #[test]
    fn whole_line_paragraph_gaps_keep_every_line_on_its_row() {
        // BOOK LINES: one blank line between paragraphs.
        let lines = page(63.0, 76.0, 5, 76.0, 200);
        let (on, all) = aligned_share(&lines, 8);
        assert_eq!(on, all, "{on} of {all}");
    }

    #[test]
    fn uneven_paragraph_gaps_align_what_they_can() {
        // A site's paragraph gap that is not a whole line (8 rows over): the lines of a screen sit on several
        // grids and a turn can match only some of them; still nothing is skipped (see the test above).
        let lines = page(63.0, 76.0, 5, 8.0, 200);
        let (on, all) = aligned_share(&lines, 8);
        assert!(on * 2 >= all, "only {on} of {all} lines clean");
    }

    #[test]
    fn bars_hiding_on_the_turn_skip_nothing() {
        // Before: the page shows between a 96-row bar and a 104-row bar; after a forward turn, the whole screen.
        let lines = page(63.0, 76.0, 1000, 0.0, 100);
        let t = Turn { scroll: 0.0, max_scroll: 1e9, top: 96.0, bottom: 896.0, next_top: 0.0, next_bottom: 1000.0 };
        let next = turn_to(&lines, &t, 1.0).unwrap();
        // The bars' height is not a whole number of lines, so this one turn cannot keep the rows; the next
        // turns, with the bars hidden, do. No line is skipped: the first new line follows the last old one.
        let last_old = whole(&lines, 0.0, 800.0).last().unwrap().baseline;
        assert_eq!(whole(&lines, next, next + 1000.0).next().unwrap().baseline, last_old + 76.0);
    }

    #[test]
    fn a_page_without_text_has_no_aligned_turn() {
        assert_eq!(turn_to(&[], &screen(0.0), 1.0), None);
        // At the top there is nothing to turn back to.
        assert_eq!(turn_to(&page(63.0, 76.0, 1000, 0.0, 30), &screen(0.0), -1.0), None);
    }
}
