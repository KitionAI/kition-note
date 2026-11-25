//! Table commands. Each rewrites only the spans it is about: a cell's segment, one line per row,
//! one delimiter cell. `format` is the one command that rewrites a whole table.

use kition_note_markdown::{Align, Index, Row, Table, sub};

use crate::{Edit, EditError, Reason, Result, eol_of, insert_block, invalid, minimal, not_found};

fn out_of_range(message: String) -> EditError {
    EditError::new(Reason::OutOfRange, message)
}

/// The table at `index` (document order, from 0).
pub fn table(ix: &Index, index: usize) -> Result<&Table> {
    ix.tables.get(index).ok_or_else(|| {
        if ix.tables.is_empty() {
            not_found("the note has no table")
        } else {
            out_of_range(format!("the note has {} tables, so there is no table {index}", ix.tables.len()))
        }
    })
}

fn row(table: &Table, index: usize) -> Result<&Row> {
    table
        .rows
        .get(index)
        .ok_or_else(|| out_of_range(format!("the table has {} rows (0 is the header), so there is no row {index}", table.rows.len())))
}

/// `|` escaped for a cell; cells are one line.
fn cell_text(text: &str) -> Result<String> {
    if text.contains(['\n', '\r']) {
        return Err(invalid("a table cell is one line; use <br> for a line break"));
    }
    let mut out = String::new();
    let mut prev = ' ';
    for c in text.chars() {
        if c == '|' && prev != '\\' {
            out.push('\\');
        }
        out.push(c);
        prev = c;
    }
    Ok(out)
}

/// A cell's text as the table shows it: `\|` unescaped.
pub fn cell_value(raw: &str) -> String {
    raw.replace("\\|", "|")
}

/// The new segment for `content` in place of `raw`: the same width and leading spaces when the
/// row was padded and it fits, one space each side otherwise (none where there was none).
fn fill(raw: &str, content: &str, padded: bool) -> String {
    let lead = raw.len() - raw.trim_start().len();
    let trail = raw.len() - raw.trim_end().len();
    let width = raw.chars().count();
    let lead_text = sub(raw, 0..lead);
    let len = content.chars().count();
    if padded && lead + len < width {
        format!("{lead_text}{content}{}", " ".repeat(width - lead - len))
    } else {
        let left = if lead == 0 { "" } else { " " };
        let right = if trail == 0 { "" } else { " " };
        format!("{left}{content}{right}")
    }
}

/// `table.setCell`: replaces one cell's content, refused with `stale` when `expected` differs.
pub fn set_cell(text: &str, ix: &Index, table_index: usize, row_index: usize, column: usize, new: &str, expected: Option<&str>) -> Result<Vec<Edit>> {
    let table = table(ix, table_index)?;
    let row = row(table, row_index)?;
    let cell = row
        .cells
        .get(column)
        .ok_or_else(|| out_of_range(format!("row {row_index} has {} cells, so there is no column {column}", row.cells.len())))?;
    let current = sub(text, cell.content.clone());
    if let Some(expected) = expected
        && cell_value(current) != expected
        && current != expected
    {
        return Err(EditError::new(Reason::Stale, format!("the cell holds {:?}, not {expected:?}", cell_value(current))));
    }
    let escaped = cell_text(new)?;
    if current == escaped {
        return Ok(Vec::new());
    }
    let raw = sub(text, cell.segment.clone());
    Ok(minimal(text, cell.segment.clone(), &fill(raw, &escaped, table.padded(text))))
}

/// The prefix of a row line before the row (a quote's `> `).
fn prefix<'a>(text: &'a str, row: &Row) -> &'a str {
    sub(text, row.line.start..row.body.start)
}

/// A new row line like `template` holding `cells` (missing ones empty), without a line ending.
fn new_row(text: &str, table: &Table, template: &Row, cells: &[String]) -> String {
    let padded = table.padded(text);
    let columns = table.columns();
    let mut out = String::from(prefix(text, template));
    if template.leading_pipe {
        out.push('|');
    }
    for column in 0..columns {
        let content = cells.get(column).map(String::as_str).unwrap_or_default();
        let width = template.cells.get(column).map_or(0, |cell| sub(text, cell.segment.clone()).chars().count());
        let segment = if padded && content.chars().count() + 2 <= width {
            format!(" {content}{}", " ".repeat(width - 1 - content.chars().count()))
        } else if content.is_empty() {
            "  ".to_owned()
        } else {
            format!(" {content} ")
        };
        out.push_str(&segment);
        if column + 1 < columns || template.trailing_pipe {
            out.push('|');
        }
    }
    if !template.trailing_pipe {
        let trimmed = out.trim_end().len();
        out.truncate(trimmed);
    }
    out
}

/// `table.insertRow`: a new body row before row `at` (1 is the first body row; one past the last
/// appends).
pub fn insert_row(text: &str, ix: &Index, table_index: usize, at: usize, cells: &[String]) -> Result<Vec<Edit>> {
    let table = table(ix, table_index)?;
    if at == 0 || at > table.rows.len() {
        return Err(out_of_range(format!("a new row goes at 1..={} (0 is the header)", table.rows.len())));
    }
    let cells = cells.iter().map(|cell| cell_text(cell)).collect::<Result<Vec<_>>>()?;
    let template = table.rows.get(1).or_else(|| table.rows.first()).ok_or_else(|| not_found("the table has no rows"))?;
    let line = new_row(text, table, template, &cells);
    match table.rows.get(at) {
        Some(before) => {
            let eol = eol_of(text, before.line.clone());
            Ok(vec![Edit::insert(before.line.start, format!("{line}{eol}"))])
        }
        None => {
            // After the last line: the delimiter row when the table has only its header.
            let last = table.lines().last().copied().ok_or_else(|| not_found("the table has no rows"))?;
            let eol = eol_of(text, last.line.clone());
            if sub(text, last.line.clone()).ends_with('\n') {
                Ok(vec![Edit::insert(last.line.end, format!("{line}{eol}"))])
            } else {
                Ok(vec![Edit::insert(last.line.end, format!("\n{line}"))])
            }
        }
    }
}

fn delimiter_cell(align: Align, width: usize) -> String {
    let width = width.max(3);
    match align {
        Align::None => "-".repeat(width),
        Align::Left => format!(":{}", "-".repeat(width - 1)),
        Align::Right => format!("{}:", "-".repeat(width - 1)),
        Align::Center => format!(":{}:", "-".repeat(width - 2)),
    }
}

/// The edit putting `segment` as column `at` of `row` (a new cell between pipes).
fn insert_segment(row: &Row, at: usize, segment: &str) -> Option<Edit> {
    if let Some(cell) = row.cells.get(at) {
        return Some(Edit::insert(cell.segment.start, format!("{segment}|")));
    }
    let last = row.cells.last()?;
    if at != row.cells.len() {
        return None;
    }
    if row.trailing_pipe {
        Some(Edit::insert(last.segment.end + 1, format!("{segment}|")))
    } else {
        // Without outer pipes a row ends at its last cell.
        Some(Edit::insert(last.segment.end, format!(" |{}", segment.trim_end())))
    }
}

/// `table.insertColumn`: a new column at `at` (one past the last appends), one edit per row line.
pub fn insert_column(text: &str, ix: &Index, table_index: usize, at: usize, header: Option<&str>, align: Align) -> Result<Vec<Edit>> {
    let table = table(ix, table_index)?;
    let columns = table.columns();
    if at > columns {
        return Err(out_of_range(format!("a new column goes at 0..={columns}")));
    }
    let header = cell_text(header.unwrap_or_default())?;
    let padded = table.padded(text);
    let width = if padded { header.chars().count().max(3) } else { header.chars().count() };
    let pad = |content: &str| {
        if padded { format!(" {content}{} ", " ".repeat(width.saturating_sub(content.chars().count()))) } else { format!(" {content} ") }
    };
    let mut edits = Vec::new();
    for (index, row) in table.lines().into_iter().enumerate() {
        let segment = match index {
            0 => pad(&header),
            1 => format!(" {} ", delimiter_cell(align, width)),
            _ => pad(""),
        };
        let segment = if !row.leading_pipe && at == 0 { segment.trim_start().to_owned() } else { segment };
        if let Some(edit) = insert_segment(row, at.min(row.cells.len()), &segment) {
            edits.push(edit);
        }
    }
    crate::normalize(edits)
}

/// `table.deleteRow`: removes a body row's line.
pub fn delete_row(ix: &Index, table_index: usize, row_index: usize) -> Result<Vec<Edit>> {
    let table = table(ix, table_index)?;
    if row_index == 0 {
        return Err(EditError::new(Reason::Refused, "the header row cannot be deleted"));
    }
    let row = row(table, row_index)?;
    Ok(vec![Edit::delete(row.line.clone())])
}

/// `table.deleteColumn`: removes the column's cell and one of its pipes from every row line.
pub fn delete_column(ix: &Index, table_index: usize, column: usize) -> Result<Vec<Edit>> {
    let table = table(ix, table_index)?;
    let columns = table.columns();
    if column >= columns {
        return Err(out_of_range(format!("the table has {columns} columns, so there is no column {column}")));
    }
    if columns == 1 {
        return Err(EditError::new(Reason::Refused, "a table keeps at least one column"));
    }
    let mut edits = Vec::new();
    for row in table.lines() {
        let Some(cell) = row.cells.get(column) else { continue };
        let has_right_pipe = column + 1 < row.cells.len() || row.trailing_pipe;
        let range = if column + 1 < row.cells.len() || (has_right_pipe && column == 0) {
            cell.segment.start..cell.segment.end + 1
        } else {
            cell.segment.start.saturating_sub(1)..cell.segment.end
        };
        edits.push(Edit::delete(range));
    }
    crate::normalize(edits)
}

/// `table.moveRow`: moves a body row's line before the row now at `to` (or to the end).
pub fn move_row(text: &str, ix: &Index, table_index: usize, from: usize, to: usize) -> Result<Vec<Edit>> {
    let table = table(ix, table_index)?;
    let body = table.rows.len().saturating_sub(1);
    if from == 0 || to == 0 {
        return Err(EditError::new(Reason::Refused, "the header row does not move"));
    }
    if from > body || to > body {
        return Err(out_of_range(format!("body rows are 1..={body}")));
    }
    if from == to {
        return Ok(Vec::new());
    }
    let moving = row(table, from)?;
    let mut line = sub(text, moving.line.clone()).to_owned();
    if !line.ends_with('\n') {
        line.push_str(eol_of(text, table.rows.first().map_or(0..0, |header| header.line.clone())));
    }
    let target = row(table, to)?;
    let (insert_at, insert) = if to > from {
        // After the row now at `to`.
        let mut after = target.line.end;
        let mut insert = line.clone();
        if !sub(text, target.line.clone()).ends_with('\n') {
            // The last row has no line ending: give it the moving row's and drop the moved one's.
            insert = format!("{}{}", eol_of(text, moving.line.clone()), line.trim_end_matches(['\n', '\r']));
            after = target.line.end;
        }
        (after, insert)
    } else {
        (target.line.start, line)
    };
    let mut delete = moving.line.clone();
    if !sub(text, moving.line.clone()).ends_with('\n') {
        // The last line moves up: its line ending is the one before it.
        delete.start = delete.start.saturating_sub(eol_of(text, row(table, from - 1)?.line.clone()).len());
    }
    crate::normalize(vec![Edit::delete(delete), Edit::insert(insert_at, insert)])
}

/// `table.moveColumn`: reorders each row's segments (padding moves with its cell).
pub fn move_column(text: &str, ix: &Index, table_index: usize, from: usize, to: usize) -> Result<Vec<Edit>> {
    let table = table(ix, table_index)?;
    let columns = table.columns();
    if from >= columns || to >= columns {
        return Err(out_of_range(format!("columns are 0..{columns}")));
    }
    if from == to {
        return Ok(Vec::new());
    }
    let mut edits = Vec::new();
    for row in table.lines() {
        if from >= row.cells.len() || to >= row.cells.len() {
            continue;
        }
        let mut segments: Vec<&str> = row.cells.iter().map(|cell| sub(text, cell.segment.clone())).collect();
        let moving = segments.remove(from);
        segments.insert(to, moving);
        // Outer cells without outer pipes keep the edge's spacing.
        let (Some(first), Some(last)) = (row.cells.first(), row.cells.last()) else { continue };
        let joined = segments.join("|");
        edits.extend(minimal(text, first.segment.start..last.segment.end, &joined));
    }
    crate::normalize(edits)
}

/// `table.align`: rewrites only the column's delimiter cell, keeping its width.
pub fn align(text: &str, ix: &Index, table_index: usize, column: usize, align: Align) -> Result<Vec<Edit>> {
    let table = table(ix, table_index)?;
    let cell = table
        .delimiter
        .cells
        .get(column)
        .ok_or_else(|| out_of_range(format!("the table has {} columns, so there is no column {column}", table.columns())))?;
    let width = sub(text, cell.content.clone()).chars().count();
    Ok(minimal(text, cell.content.clone(), &delimiter_cell(align, width)))
}

/// `table.format`: rewrites the whole table with padded, aligned columns; a row with more cells
/// than the header gets empty header cells, so no cell is lost.
pub fn format(text: &str, ix: &Index, table_index: usize) -> Result<Vec<Edit>> {
    let table = table(ix, table_index)?;
    let columns = table.rows.iter().map(|row| row.cells.len()).max().unwrap_or(0);
    let contents: Vec<Vec<String>> = table
        .rows
        .iter()
        .map(|row| {
            (0..columns).map(|column| row.cells.get(column).map(|cell| sub(text, cell.content.clone()).to_owned()).unwrap_or_default()).collect()
        })
        .collect();
    let widths: Vec<usize> = (0..columns)
        .map(|column| contents.iter().map(|row| row.get(column).map_or(0, |cell| cell.chars().count())).max().unwrap_or(0).max(3))
        .collect();
    let header = table.rows.first().ok_or_else(|| not_found("the table has no rows"))?;
    let lead = prefix(text, header);
    let eol = eol_of(text, header.line.clone());
    let render = |cells: &[String]| {
        let parts: Vec<String> =
            cells.iter().zip(&widths).map(|(cell, width)| format!(" {cell}{} ", " ".repeat(width - cell.chars().count()))).collect();
        format!("{lead}|{}|", parts.join("|"))
    };
    let mut lines = Vec::new();
    for (index, row) in contents.iter().enumerate() {
        lines.push(render(row));
        if index == 0 {
            let delimiter: Vec<String> = widths
                .iter()
                .enumerate()
                .map(|(column, width)| delimiter_cell(table.align.get(column).copied().unwrap_or(Align::None), *width))
                .collect();
            lines.push(format!("{lead}|{}|", delimiter.iter().map(|cell| format!(" {cell} ")).collect::<Vec<_>>().join("|")));
        }
    }
    let start = header.line.start;
    let last = table.rows.last().map_or(header.line.clone(), |row| row.line.clone());
    let end = if sub(text, last.clone()).ends_with('\n') { last.end - eol.len() } else { last.end };
    Ok(minimal(text, start..end, &lines.join(eol)))
}

/// Whether a new block can go in at the line start `at`: not inside the front matter (nor before
/// it), a code block or a table, where it would be properties, code or rows of the other table.
pub fn block_place(ix: &Index, at: usize, line: usize) -> Result<()> {
    let inside = |range: &std::ops::Range<usize>| range.start < at && at < range.end;
    if ix.frontmatter.as_ref().is_some_and(|found| at < found.range.end) {
        return Err(invalid(format!("line {line} is in the front matter; a table goes after it")));
    }
    if ix.code.iter().any(|code| inside(&code.range)) {
        return Err(invalid(format!("line {line} is inside a code block; give the line of its opening fence or one after it")));
    }
    if ix.tables.iter().any(|table| inside(&table.range)) {
        return Err(invalid(format!("line {line} is inside a table; give the table's first line or one after it")));
    }
    Ok(())
}

/// `table.add`: a new table at a line start, with blank lines around it as Markdown needs.
pub fn create(at: usize, text: &str, columns: usize, rows: usize, header: &[String]) -> Result<Vec<Edit>> {
    if columns == 0 || columns > 64 {
        return Err(invalid("columns must be 1 to 64"));
    }
    if rows > 1000 {
        return Err(invalid("rows must be at most 1000"));
    }
    let header: Vec<String> = (0..columns)
        .map(|column| header.get(column).map(|cell| cell_text(cell)).transpose().map(|cell| cell.unwrap_or_else(|| format!("Column {}", column + 1))))
        .collect::<Result<_>>()?;
    let mut lines = vec![format!("| {} |", header.join(" | ")), format!("|{}|", vec![" --- "; columns].join("|"))];
    for _ in 0..rows {
        lines.push(format!("|{}|", vec!["  "; columns].join("|")));
    }
    Ok(vec![insert_block(text, at, &lines.join("\n"))])
}

#[cfg(test)]
mod tests {
    use kition_note_markdown::index;

    use super::*;
    use crate::apply;

    fn cell(text: &str, row: usize, column: usize, new: &str) -> String {
        apply(text, &set_cell(text, &index(text), 0, row, column, new, None).unwrap())
    }

    #[test]
    fn set_cell_keeps_the_rows_style() {
        assert_eq!(cell("| a | b |\n|---|---|\n| 1 | 2 |\n", 1, 0, "x"), "| a | b |\n|---|---|\n| x | 2 |\n");
        assert_eq!(cell("| aa   | b |\n| ---- | - |\n| 1    | 2 |\n", 1, 0, "xy"), "| aa   | b |\n| ---- | - |\n| xy   | 2 |\n");
        assert_eq!(cell("| aa   | b |\n| ---- | - |\n| 1    | 2 |\n", 1, 0, "longer"), "| aa   | b |\n| ---- | - |\n| longer | 2 |\n");
        assert_eq!(cell("a | b\n--|--\n1 | 2\n", 1, 1, "p|q"), "a | b\n--|--\n1 | p\\|q\n");
        assert_eq!(cell("|a|b|\n|-|-|\n|1|2|\n", 1, 1, "z"), "|a|b|\n|-|-|\n|1|z|\n");
        assert_eq!(cell("| `a\\|b` | c |\n|---|---|\n| 1 | 2 |\n", 0, 1, "d"), "| `a\\|b` | d |\n|---|---|\n| 1 | 2 |\n");
        assert_eq!(cell("| a |  |\n|---|---|\n|  | 2 |\n", 1, 0, "n"), "| a |  |\n|---|---|\n| n | 2 |\n");
    }

    #[test]
    fn set_cell_guards_and_ranges() {
        let text = "| a | b |\n|---|---|\n| 1 | 2 |\n";
        let ix = index(text);
        assert_eq!(set_cell(text, &ix, 0, 1, 0, "x", Some("9")).unwrap_err().reason, Reason::Stale);
        assert!(set_cell(text, &ix, 0, 1, 0, "x", Some("1")).is_ok());
        assert_eq!(set_cell(text, &ix, 1, 1, 0, "x", None).unwrap_err().reason, Reason::OutOfRange);
        assert_eq!(set_cell(text, &ix, 0, 5, 0, "x", None).unwrap_err().reason, Reason::OutOfRange);
        assert_eq!(set_cell(text, &ix, 0, 1, 0, "a\nb", None).unwrap_err().reason, Reason::InvalidParams);
        assert_eq!(set_cell("x", &index("x"), 0, 0, 0, "y", None).unwrap_err().reason, Reason::NotFound);
    }

    #[test]
    fn rows_are_added_moved_and_deleted_one_line_each() {
        let text = "| a | b |\n|---|---|\n| 1 | 2 |\n| 3 | 4 |\n";
        let ix = index(text);
        assert_eq!(apply(text, &insert_row(text, &ix, 0, 3, &["5".into()]).unwrap()), "| a | b |\n|---|---|\n| 1 | 2 |\n| 3 | 4 |\n| 5 |  |\n");
        assert_eq!(apply(text, &insert_row(text, &ix, 0, 1, &[]).unwrap()), "| a | b |\n|---|---|\n|  |  |\n| 1 | 2 |\n| 3 | 4 |\n");
        assert_eq!(apply(text, &delete_row(&ix, 0, 1).unwrap()), "| a | b |\n|---|---|\n| 3 | 4 |\n");
        assert!(delete_row(&ix, 0, 0).is_err());
        assert_eq!(apply(text, &move_row(text, &ix, 0, 2, 1).unwrap()), "| a | b |\n|---|---|\n| 3 | 4 |\n| 1 | 2 |\n");
        assert_eq!(apply(text, &move_row(text, &ix, 0, 1, 2).unwrap()), "| a | b |\n|---|---|\n| 3 | 4 |\n| 1 | 2 |\n");
        let open = "| a |\n|---|\n| 1 |\n| 2 |";
        let ix = index(open);
        assert_eq!(apply(open, &move_row(open, &ix, 0, 2, 1).unwrap()), "| a |\n|---|\n| 2 |\n| 1 |");
        assert_eq!(apply(open, &move_row(open, &ix, 0, 1, 2).unwrap()), "| a |\n|---|\n| 2 |\n| 1 |");
        assert_eq!(apply(open, &insert_row(open, &ix, 0, 3, &["3".into()]).unwrap()), "| a |\n|---|\n| 1 |\n| 2 |\n| 3 |");
    }

    #[test]
    fn padded_rows_stay_padded() {
        let text = "| Item | Owner |\n| ---- | ----- |\n| Ship | Ann   |\n";
        let ix = index(text);
        assert_eq!(
            apply(text, &insert_row(text, &ix, 0, 2, &["Fix".into(), "Bo".into()]).unwrap()),
            "| Item | Owner |\n| ---- | ----- |\n| Ship | Ann   |\n| Fix  | Bo    |\n"
        );
    }

    #[test]
    fn columns_are_added_moved_aligned_and_deleted() {
        let text = "| a | b |\n|---|---|\n| 1 | 2 |\n";
        let ix = index(text);
        assert_eq!(apply(text, &insert_column(text, &ix, 0, 1, Some("x"), Align::None).unwrap()), "| a | x | b |\n|---| --- |---|\n| 1 |  | 2 |\n");
        assert_eq!(apply(text, &insert_column(text, &ix, 0, 2, Some("x"), Align::Right).unwrap()), "| a | b | x |\n|---|---| --: |\n| 1 | 2 |  |\n");
        assert_eq!(apply(text, &delete_column(&ix, 0, 0).unwrap()), "| b |\n|---|\n| 2 |\n");
        assert_eq!(apply(text, &delete_column(&ix, 0, 1).unwrap()), "| a |\n|---|\n| 1 |\n");
        assert_eq!(apply(text, &move_column(text, &ix, 0, 0, 1).unwrap()), "| b | a |\n|---|---|\n| 2 | 1 |\n");
        assert_eq!(apply(text, &align(text, &ix, 0, 1, Align::Center).unwrap()), "| a | b |\n|---|:-:|\n| 1 | 2 |\n");
        let bare = "a | b\n--|--\n1 | 2\n";
        let ix = index(bare);
        assert_eq!(apply(bare, &insert_column(bare, &ix, 0, 2, Some("c"), Align::None).unwrap()), "a | b | c\n--|-- | ---\n1 | 2 |\n");
        assert_eq!(apply(bare, &delete_column(&ix, 0, 0).unwrap()), " b\n--\n 2\n");
    }

    #[test]
    fn format_rewrites_the_whole_table_and_keeps_every_cell() {
        let text = "| a | b |\n| - | :- |\n| 1 | 2 | 3 |\n\nafter\n";
        let ix = index(text);
        assert_eq!(apply(text, &format(text, &ix, 0).unwrap()), "| a   | b   |     |\n| --- | :-- | --- |\n| 1   | 2   | 3   |\n\nafter\n");
    }

    #[test]
    fn create_writes_a_block() {
        let text = "Intro\nnext\n";
        assert_eq!(apply(text, &create(6, text, 2, 1, &["A".into()]).unwrap()), "Intro\n\n| A | Column 2 |\n| --- | --- |\n|  |  |\n\nnext\n");
    }
}
