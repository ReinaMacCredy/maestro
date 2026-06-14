//! Space-padded column alignment for list-style plain-text output.

use std::fmt::Write as _;

/// Render rows as aligned columns: each column as wide as its widest cell
/// (headers included), two spaces between columns, no trailing padding on the
/// last column. An empty `headers` slice renders the rows without a header
/// line. Ends with a trailing newline when anything was rendered.
pub fn render_table(headers: &[&str], rows: &[Vec<String>]) -> String {
    let columns = rows
        .iter()
        .map(Vec::len)
        .chain(std::iter::once(headers.len()))
        .max()
        .unwrap_or(0);
    let mut widths = vec![0usize; columns];
    for (index, header) in headers.iter().enumerate() {
        widths[index] = header.len();
    }
    for row in rows {
        for (index, cell) in row.iter().enumerate() {
            widths[index] = widths[index].max(cell.len());
        }
    }

    let mut out = String::new();
    if !headers.is_empty() {
        push_row(&mut out, headers.iter().copied(), &widths);
    }
    for row in rows {
        push_row(&mut out, row.iter().map(String::as_str), &widths);
    }
    out
}

fn push_row<'a>(out: &mut String, cells: impl Iterator<Item = &'a str>, widths: &[usize]) {
    let cells: Vec<&str> = cells.collect();
    for (index, cell) in cells.iter().enumerate() {
        if index + 1 == cells.len() {
            out.push_str(cell);
        } else {
            write!(out, "{cell:<width$}  ", width = widths[index])
                .expect("invariant: writing to String cannot fail");
        }
    }
    out.push('\n');
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn columns_pad_to_the_widest_cell_including_the_header() {
        let rows = vec![
            vec![
                "card-167cfb".to_string(),
                "locked".to_string(),
                "t".to_string(),
            ],
            vec![
                "c-1".to_string(),
                "open".to_string(),
                "title two".to_string(),
            ],
        ];
        let table = render_table(&["ID", "STATUS", "TITLE"], &rows);
        assert_eq!(
            table,
            "ID           STATUS  TITLE\n\
             card-167cfb  locked  t\n\
             c-1          open    title two\n"
        );
    }

    #[test]
    fn empty_headers_render_rows_only() {
        let rows = vec![vec!["a".to_string(), "bb".to_string()]];
        assert_eq!(render_table(&[], &rows), "a  bb\n");
    }
}
