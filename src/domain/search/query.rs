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
    pub status: Option<String>,
    pub feature: Option<String>,
    pub runtime: Option<String>,
    pub event: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ParsedQuery {
    pub raw: String,
    pub terms: Vec<String>,
    pub filters: QueryFilters,
    pub case_mode: CaseMode,
    pub explicit_filter_overrides: Vec<String>,
}

pub fn parse(raw: &str) -> Result<ParsedQuery, SearchDiagnostic> {
    let atoms = tokenize(raw)?;
    let mut terms = Vec::new();
    let mut filters = QueryFilters::default();
    let mut case_mode = CaseMode::Auto;
    let mut overrides = Vec::new();

    for atom in atoms {
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
                "file" | "lang" | "sym" => {
                    return Err(SearchDiagnostic::error(
                        "source_filter_unavailable",
                        format!(
                            "{name}: is a source-corpus filter; source grep is not enabled yet"
                        ),
                    ));
                }
                _ => terms.push(atom),
            }
        } else if atom.starts_with('/') {
            return Err(SearchDiagnostic::error(
                "source_filter_unavailable",
                "/regex/ atoms require the source shard",
            ));
        } else if atom == "or"
            || atom == "OR"
            || atom.starts_with('-')
            || atom == "("
            || atom == ")"
        {
            return Err(SearchDiagnostic::error(
                "query_grammar_unavailable",
                "boolean grammar ships with the ranking/output slice; use plain memory terms here",
            ));
        } else {
            terms.push(atom);
        }
    }

    if terms.is_empty() {
        return Err(SearchDiagnostic::error(
            "empty_query",
            "maestro grep needs at least one text term in the memory slice",
        ));
    }

    Ok(ParsedQuery {
        raw: raw.to_string(),
        terms,
        filters,
        case_mode,
        explicit_filter_overrides: overrides,
    })
}

pub fn literal_case_sensitive(query: &ParsedQuery) -> bool {
    match query.case_mode {
        CaseMode::Yes => true,
        CaseMode::No => false,
        CaseMode::Auto => query.terms.iter().any(|term| {
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
}
