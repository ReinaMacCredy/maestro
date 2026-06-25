use crate::domain::search::types::SearchDiagnostic;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CaseMode {
    Yes,
    No,
    Auto,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct QueryFilters {
    pub corpus: Option<String>,
    pub kinds: Vec<String>,
    pub file_globs: Vec<String>,
    pub excluded_file_globs: Vec<String>,
    pub lang: Option<String>,
    pub sym: Option<String>,
    pub status: Option<String>,
    pub feature: Option<String>,
    pub runtime: Option<String>,
    pub event: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum QueryAtom {
    Literal(String),
    Regex(String),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum QueryExpr {
    Atom(QueryAtom),
    Not(Box<QueryExpr>),
    And(Vec<QueryExpr>),
    Or(Vec<QueryExpr>),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ParsedQuery {
    pub raw: String,
    pub terms: Vec<String>,
    pub regexes: Vec<String>,
    pub excluded_terms: Vec<String>,
    pub filters: QueryFilters,
    pub case_mode: CaseMode,
    pub explicit_filter_overrides: Vec<String>,
    pub expr: QueryExpr,
}

pub fn parse(raw: &str) -> Result<ParsedQuery, SearchDiagnostic> {
    let atoms = tokenize(raw)?;
    let mut terms = Vec::new();
    let mut regexes = Vec::new();
    let mut excluded_terms = Vec::new();
    let mut filters = QueryFilters::default();
    let mut case_mode = CaseMode::Auto;
    let mut overrides = Vec::new();
    let mut expr_atoms = Vec::new();
    let mut negate_next = false;

    for atom in atoms {
        if atom == "-" {
            if negate_next {
                return Err(SearchDiagnostic::error(
                    "parse_error",
                    "double negation is not supported",
                ));
            }
            negate_next = true;
            continue;
        }

        if atom == "(" || atom == ")" || atom == "or" || atom == "OR" {
            if negate_next && (atom == "or" || atom == "OR") {
                return Err(SearchDiagnostic::error(
                    "parse_error",
                    "negation must precede a term, regex, filter, or parenthesized expression",
                ));
            }
            if negate_next && atom == "(" {
                expr_atoms.push("-".to_string());
                negate_next = false;
            }
            expr_atoms.push(atom);
            continue;
        }

        if let Some((name, value)) = atom.split_once(':') {
            match name {
                "repo" | "branch" | "fork" | "public" | "archived" => {
                    return Err(SearchDiagnostic::error(
                        "unsupported_filter",
                        format!("unsupported filter {name}:"),
                    ));
                }
                "corpus" => {
                    if value != "memory" && value != "source" {
                        return Err(SearchDiagnostic::error(
                            "invalid_filter",
                            format!("unsupported corpus {value:?}; expected memory or source"),
                        ));
                    }
                    filters.corpus = Some(value.to_string());
                    push_once(&mut overrides, "corpus");
                }
                "type" => {
                    filters.kinds.push(value.to_string());
                    push_once(&mut overrides, "type");
                }
                "case" => {
                    case_mode = match value {
                        "yes" => CaseMode::Yes,
                        "no" => CaseMode::No,
                        "auto" => CaseMode::Auto,
                        _ => {
                            return Err(SearchDiagnostic::error(
                                "invalid_filter",
                                "case: must be yes, no, or auto",
                            ));
                        }
                    };
                    push_once(&mut overrides, "case");
                }
                "status" => {
                    filters.status = Some(value.to_string());
                    push_once(&mut overrides, "status");
                }
                "feature" => {
                    filters.feature = Some(value.to_string());
                    push_once(&mut overrides, "feature");
                }
                "runtime" => {
                    filters.runtime = Some(value.to_string());
                    push_once(&mut overrides, "runtime");
                }
                "event" => {
                    filters.event = Some(value.to_string());
                    push_once(&mut overrides, "event");
                }
                "file" => {
                    if negate_next {
                        filters.excluded_file_globs.push(value.to_string());
                        negate_next = false;
                    } else {
                        filters.file_globs.push(value.to_string());
                    }
                    push_once(&mut overrides, "file");
                }
                "lang" => {
                    if negate_next {
                        return Err(SearchDiagnostic::error(
                            "parse_error",
                            "-lang: is not supported; use lang:<language> with positive source filters",
                        ));
                    }
                    filters.lang = Some(value.to_string());
                    push_once(&mut overrides, "lang");
                }
                "sym" => {
                    if negate_next {
                        return Err(SearchDiagnostic::error(
                            "parse_error",
                            "-sym: is not supported in this slice",
                        ));
                    }
                    filters.sym = Some(value.to_string());
                    push_once(&mut overrides, "sym");
                }
                _ => push_literal_atom(
                    &mut terms,
                    &mut excluded_terms,
                    &mut expr_atoms,
                    atom,
                    &mut negate_next,
                ),
            }
        } else if atom.starts_with('/') && atom.ends_with('/') && atom.len() >= 2 {
            let pattern = atom[1..atom.len() - 1].to_string();
            if pattern.is_empty() {
                return Err(SearchDiagnostic::error("parse_error", "empty regex atom"));
            }
            if negate_next {
                expr_atoms.push("-".to_string());
                negate_next = false;
            } else {
                regexes.push(pattern.clone());
            }
            expr_atoms.push(format!("/{pattern}/"));
        } else {
            push_literal_atom(
                &mut terms,
                &mut excluded_terms,
                &mut expr_atoms,
                atom,
                &mut negate_next,
            );
        }
    }

    if negate_next {
        return Err(SearchDiagnostic::error(
            "parse_error",
            "negation must precede a term, regex, filter, or parenthesized expression",
        ));
    }
    let expr = if expr_atoms.is_empty() && filters.sym.is_some() {
        QueryExpr::And(Vec::new())
    } else {
        parse_expr(&expr_atoms)?
    };

    if terms.is_empty() && regexes.is_empty() && filters.sym.is_none() {
        return Err(SearchDiagnostic::error(
            "empty_query",
            "maestro grep needs at least one text term, /regex/ atom, or sym:<name>",
        ));
    }

    Ok(ParsedQuery {
        raw: raw.to_string(),
        terms,
        regexes,
        excluded_terms,
        filters,
        case_mode,
        explicit_filter_overrides: overrides,
        expr,
    })
}

pub fn literal_case_sensitive(query: &ParsedQuery) -> bool {
    match query.case_mode {
        CaseMode::Yes => true,
        CaseMode::No => false,
        CaseMode::Auto => query.terms.iter().chain(query.regexes.iter()).any(|term| {
            term.chars()
                .any(|ch| ch.is_uppercase() || (ch.is_alphabetic() && !ch.is_lowercase()))
        }),
    }
}

fn push_once(values: &mut Vec<String>, value: &str) {
    if !values.iter().any(|existing| existing == value) {
        values.push(value.to_string());
    }
}

fn push_literal_atom(
    terms: &mut Vec<String>,
    excluded_terms: &mut Vec<String>,
    expr_atoms: &mut Vec<String>,
    atom: String,
    negate_next: &mut bool,
) {
    if *negate_next {
        excluded_terms.push(atom.clone());
        expr_atoms.push("-".to_string());
        *negate_next = false;
    } else {
        terms.push(atom.clone());
    }
    expr_atoms.push(atom);
}

fn tokenize(raw: &str) -> Result<Vec<String>, SearchDiagnostic> {
    let mut atoms = Vec::new();
    let mut current = String::new();
    let mut quote = false;
    let mut regex = false;
    let mut escaped = false;

    for ch in raw.chars() {
        if escaped {
            current.push(ch);
            escaped = false;
            continue;
        }

        if quote {
            match ch {
                '\\' => escaped = true,
                '"' => quote = false,
                _ => current.push(ch),
            }
            continue;
        }

        if regex {
            current.push(ch);
            match ch {
                '\\' => escaped = true,
                '/' => regex = false,
                _ => {}
            }
            continue;
        }

        match ch {
            '"' => quote = true,
            '(' | ')' => {
                if !current.is_empty() {
                    atoms.push(std::mem::take(&mut current));
                }
                atoms.push(ch.to_string());
            }
            '-' if current.is_empty() => atoms.push("-".to_string()),
            '/' if current.is_empty() => {
                regex = true;
                current.push(ch);
            }
            ch if ch.is_whitespace() => {
                if !current.is_empty() {
                    atoms.push(std::mem::take(&mut current));
                }
            }
            _ => current.push(ch),
        }
    }

    if quote {
        return Err(SearchDiagnostic::error(
            "parse_error",
            "unclosed quoted string",
        ));
    }
    if regex {
        return Err(SearchDiagnostic::error(
            "parse_error",
            "unclosed regex atom",
        ));
    }
    if escaped {
        current.push('\\');
    }
    if !current.is_empty() {
        atoms.push(current);
    }
    Ok(atoms)
}

fn parse_expr(atoms: &[String]) -> Result<QueryExpr, SearchDiagnostic> {
    let mut parser = ExprParser { atoms, cursor: 0 };
    let expr = parser.parse_or()?;
    if parser.cursor != atoms.len() {
        let token = &atoms[parser.cursor];
        return Err(SearchDiagnostic::error(
            "parse_error",
            format!("unexpected token {token:?}"),
        ));
    }
    Ok(expr)
}

struct ExprParser<'a> {
    atoms: &'a [String],
    cursor: usize,
}

impl ExprParser<'_> {
    fn parse_or(&mut self) -> Result<QueryExpr, SearchDiagnostic> {
        let mut clauses = vec![self.parse_and()?];
        while self.peek_is("or") || self.peek_is("OR") {
            self.cursor += 1;
            clauses.push(self.parse_and()?);
        }
        Ok(if clauses.len() == 1 {
            clauses.remove(0)
        } else {
            QueryExpr::Or(clauses)
        })
    }

    fn parse_and(&mut self) -> Result<QueryExpr, SearchDiagnostic> {
        let mut clauses = Vec::new();
        while self.cursor < self.atoms.len()
            && !self.peek_is(")")
            && !self.peek_is("or")
            && !self.peek_is("OR")
        {
            clauses.push(self.parse_unary()?);
        }
        if clauses.is_empty() {
            return Err(SearchDiagnostic::error(
                "parse_error",
                "expected term, /regex/, or parenthesized expression",
            ));
        }
        Ok(if clauses.len() == 1 {
            clauses.remove(0)
        } else {
            QueryExpr::And(clauses)
        })
    }

    fn parse_unary(&mut self) -> Result<QueryExpr, SearchDiagnostic> {
        if self.peek_is("-") {
            self.cursor += 1;
            return Ok(QueryExpr::Not(Box::new(self.parse_unary()?)));
        }
        self.parse_primary()
    }

    fn parse_primary(&mut self) -> Result<QueryExpr, SearchDiagnostic> {
        if self.peek_is("(") {
            self.cursor += 1;
            let expr = self.parse_or()?;
            if !self.peek_is(")") {
                return Err(SearchDiagnostic::error(
                    "parse_error",
                    "unclosed parenthesized expression",
                ));
            }
            self.cursor += 1;
            return Ok(expr);
        }
        if self.peek_is(")") {
            return Err(SearchDiagnostic::error(
                "parse_error",
                "unexpected closing parenthesis",
            ));
        }
        let Some(atom) = self.atoms.get(self.cursor) else {
            return Err(SearchDiagnostic::error(
                "parse_error",
                "expected term or /regex/",
            ));
        };
        self.cursor += 1;
        if atom.starts_with('/') && atom.ends_with('/') && atom.len() >= 2 {
            Ok(QueryExpr::Atom(QueryAtom::Regex(
                atom[1..atom.len() - 1].to_string(),
            )))
        } else {
            Ok(QueryExpr::Atom(QueryAtom::Literal(atom.clone())))
        }
    }

    fn peek_is(&self, expected: &str) -> bool {
        self.atoms
            .get(self.cursor)
            .is_some_and(|atom| atom == expected)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_archived_as_filter_not_metadata() {
        let error = parse("runtime archived:true").expect_err("archived: must be unsupported");
        assert_eq!(error.code, "unsupported_filter");
        assert!(error.message.contains("archived:"));
    }

    #[test]
    fn parses_memory_filters_and_quotes() {
        let parsed = parse(r#"type:decision corpus:memory "agent runtime""#)
            .expect("memory filters and quoted terms should parse");
        assert_eq!(parsed.terms, vec!["agent runtime"]);
        assert!(parsed.regexes.is_empty());
        assert_eq!(parsed.filters.corpus.as_deref(), Some("memory"));
        assert_eq!(parsed.filters.kinds, vec!["decision"]);
        assert_eq!(
            parsed.explicit_filter_overrides,
            vec!["type".to_string(), "corpus".to_string()]
        );
    }

    #[test]
    fn case_auto_becomes_sensitive_for_mixed_case_terms() {
        let parsed = parse("HTTPServer").expect("plain term should parse");
        assert!(literal_case_sensitive(&parsed));
        let parsed = parse("httpserver").expect("plain term should parse");
        assert!(!literal_case_sensitive(&parsed));
    }

    #[test]
    fn parses_source_filters_regex_negation_and_or() {
        let parsed = parse(r#"(/fn\s+\w+/ or HTTPServer) -file:tests/* lang:rust corpus:source"#)
            .expect("source query should parse");
        assert_eq!(parsed.regexes, vec![r"fn\s+\w+"]);
        assert_eq!(parsed.terms, vec!["HTTPServer"]);
        assert_eq!(parsed.filters.file_globs, Vec::<String>::new());
        assert_eq!(parsed.filters.excluded_file_globs, vec!["tests/*"]);
        assert_eq!(parsed.filters.lang.as_deref(), Some("rust"));
        assert!(matches!(parsed.expr, QueryExpr::Or(_)));
    }

    #[test]
    fn parses_sym_only_source_query() {
        let parsed = parse("sym:TaskRegistry corpus:source").expect("sym query should parse");
        assert_eq!(parsed.filters.sym.as_deref(), Some("TaskRegistry"));
        assert!(parsed.terms.is_empty());
        assert!(parsed.regexes.is_empty());
    }

    #[test]
    fn reports_parenthesis_errors() {
        let error = parse("(runtime or source").expect_err("unclosed parenthesis should fail");
        assert_eq!(error.code, "parse_error");
        assert!(error.message.contains("unclosed"));
    }
}
