use serde::{Deserialize, Serialize};
use tree_sitter::{Language, Node, Parser};

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct OutlineEntry {
    pub file: String,
    pub range: OutlineRange,
    pub name: String,
    pub outline_kind: String,
    pub signature: String,
    pub parent: Option<String>,
    pub members: Vec<String>,
    pub visibility: Option<String>,
    pub exported: bool,
    pub imported: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct OutlineRange {
    pub start_line: u64,
    pub end_line: u64,
    pub start_byte: usize,
    pub end_byte: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OutlineExtractorHealth {
    pub supported_languages: Vec<&'static str>,
}

pub fn extractor_health() -> OutlineExtractorHealth {
    OutlineExtractorHealth {
        supported_languages: vec!["rust", "typescript", "javascript", "python", "markdown"],
    }
}

pub fn extract_outline(file: &str, language: &str, contents: &str) -> Vec<OutlineEntry> {
    let Some(tree) = parse(language, contents) else {
        return Vec::new();
    };
    let mut entries = Vec::new();
    match language {
        "rust" => walk_rust(file, contents, tree.root_node(), None, &mut entries),
        "typescript" | "javascript" => {
            walk_js_like(file, contents, tree.root_node(), None, false, &mut entries)
        }
        "python" => walk_python(file, contents, tree.root_node(), None, &mut entries),
        "markdown" => walk_markdown(file, contents, tree.root_node(), &mut entries),
        _ => {}
    }
    entries
}

fn parse(language: &str, contents: &str) -> Option<tree_sitter::Tree> {
    let language: Language = match language {
        "rust" => tree_sitter_rust::LANGUAGE.into(),
        "typescript" => tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
        "javascript" => tree_sitter_javascript::LANGUAGE.into(),
        "python" => tree_sitter_python::LANGUAGE.into(),
        "markdown" => tree_sitter_md::LANGUAGE.into(),
        _ => return None,
    };
    let mut parser = Parser::new();
    parser.set_language(&language).ok()?;
    parser.parse(contents, None)
}

fn walk_rust(
    file: &str,
    contents: &str,
    node: Node<'_>,
    parent: Option<String>,
    entries: &mut Vec<OutlineEntry>,
) {
    let mut next_parent = parent.clone();
    match node.kind() {
        "struct_item" => {
            if let Some(name) = node_name(contents, node) {
                entries.push(entry(
                    file,
                    contents,
                    node,
                    EntryDraft::new(name.clone(), "struct")
                        .parent(parent.clone())
                        .members(rust_members(contents, node))
                        .visibility(rust_visibility(contents, node))
                        .exported(rust_exported(contents, node)),
                ));
                next_parent = Some(name);
            }
        }
        "enum_item" => {
            if let Some(name) = node_name(contents, node) {
                entries.push(entry(
                    file,
                    contents,
                    node,
                    EntryDraft::new(name.clone(), "enum")
                        .parent(parent.clone())
                        .members(rust_named_children(contents, node, "enum_variant"))
                        .visibility(rust_visibility(contents, node))
                        .exported(rust_exported(contents, node)),
                ));
                next_parent = Some(name);
            }
        }
        "trait_item" => {
            if let Some(name) = node_name(contents, node) {
                entries.push(entry(
                    file,
                    contents,
                    node,
                    EntryDraft::new(name.clone(), "trait")
                        .parent(parent.clone())
                        .members(rust_named_children(contents, node, "function_item"))
                        .visibility(rust_visibility(contents, node))
                        .exported(rust_exported(contents, node)),
                ));
                next_parent = Some(name);
            }
        }
        "impl_item" => {
            if let Some(name) = node_child_text(contents, node, "type") {
                let name = clean_identifier(&name);
                entries.push(entry(
                    file,
                    contents,
                    node,
                    EntryDraft::new(name.clone(), "impl")
                        .parent(parent.clone())
                        .members(rust_named_children(contents, node, "function_item")),
                ));
                next_parent = Some(name);
            }
        }
        "function_item" => {
            if let Some(name) = node_name(contents, node) {
                let kind = if parent.is_some() {
                    "method"
                } else {
                    "function"
                };
                entries.push(entry(
                    file,
                    contents,
                    node,
                    EntryDraft::new(name, kind)
                        .parent(parent.clone())
                        .visibility(rust_visibility(contents, node))
                        .exported(rust_exported(contents, node)),
                ));
            }
        }
        "field_declaration" => {
            if let Some(name) = node_child_text(contents, node, "name") {
                entries.push(entry(
                    file,
                    contents,
                    node,
                    EntryDraft::new(clean_identifier(&name), "field")
                        .parent(parent.clone())
                        .visibility(rust_visibility(contents, node))
                        .exported(rust_exported(contents, node)),
                ));
            }
        }
        "mod_item" => {
            if let Some(name) = node_name(contents, node) {
                entries.push(entry(
                    file,
                    contents,
                    node,
                    EntryDraft::new(name, "module")
                        .parent(parent.clone())
                        .visibility(rust_visibility(contents, node))
                        .exported(rust_exported(contents, node)),
                ));
            }
        }
        "use_declaration" => {
            let name = signature(contents, node)
                .trim_start_matches("use ")
                .trim_end_matches(';')
                .to_string();
            entries.push(entry(
                file,
                contents,
                node,
                EntryDraft::new(name, "import")
                    .parent(parent.clone())
                    .visibility(rust_visibility(contents, node))
                    .exported(rust_exported(contents, node))
                    .imported(true),
            ));
        }
        _ => {}
    }

    for child in named_children(node) {
        walk_rust(file, contents, child, next_parent.clone(), entries);
    }
}

fn walk_js_like(
    file: &str,
    contents: &str,
    node: Node<'_>,
    parent: Option<String>,
    exported: bool,
    entries: &mut Vec<OutlineEntry>,
) {
    let mut next_parent = parent.clone();
    let mut child_exported = exported;
    match node.kind() {
        "export_statement" => {
            child_exported = true;
            if let Some(name) = first_named_identifier(contents, node) {
                entries.push(entry(
                    file,
                    contents,
                    node,
                    EntryDraft::new(name, "export")
                        .parent(parent.clone())
                        .visibility(Some("export".to_string()))
                        .exported(true),
                ));
            }
        }
        "import_statement" => {
            let name =
                first_named_identifier(contents, node).unwrap_or_else(|| signature(contents, node));
            entries.push(entry(
                file,
                contents,
                node,
                EntryDraft::new(name, "import")
                    .parent(parent.clone())
                    .imported(true),
            ));
        }
        "function_declaration" | "generator_function_declaration" => {
            if let Some(name) = node_name(contents, node) {
                entries.push(entry(
                    file,
                    contents,
                    node,
                    EntryDraft::new(name, "function")
                        .parent(parent.clone())
                        .visibility(js_visibility(exported))
                        .exported(child_exported),
                ));
            }
        }
        "class_declaration" | "abstract_class_declaration" => {
            if let Some(name) = node_name(contents, node) {
                entries.push(entry(
                    file,
                    contents,
                    node,
                    EntryDraft::new(name.clone(), "struct")
                        .parent(parent.clone())
                        .members(js_named_children(contents, node, "method_definition"))
                        .visibility(js_visibility(exported))
                        .exported(child_exported),
                ));
                next_parent = Some(name);
            }
        }
        "method_definition" => {
            if let Some(name) = node_name(contents, node) {
                entries.push(entry(
                    file,
                    contents,
                    node,
                    EntryDraft::new(name, "method")
                        .parent(parent.clone())
                        .visibility(js_visibility(exported))
                        .exported(child_exported),
                ));
            }
        }
        _ => {}
    }

    for child in named_children(node) {
        walk_js_like(
            file,
            contents,
            child,
            next_parent.clone(),
            child_exported,
            entries,
        );
    }
}

fn walk_python(
    file: &str,
    contents: &str,
    node: Node<'_>,
    parent: Option<String>,
    entries: &mut Vec<OutlineEntry>,
) {
    let mut next_parent = parent.clone();
    match node.kind() {
        "class_definition" => {
            if let Some(name) = node_name(contents, node) {
                entries.push(entry(
                    file,
                    contents,
                    node,
                    EntryDraft::new(name.clone(), "struct")
                        .parent(parent.clone())
                        .members(python_named_children(contents, node, "function_definition"))
                        .exported(!name.starts_with('_')),
                ));
                next_parent = Some(name);
            }
        }
        "function_definition" => {
            if let Some(name) = node_name(contents, node) {
                let kind = if parent.is_some() {
                    "method"
                } else {
                    "function"
                };
                entries.push(entry(
                    file,
                    contents,
                    node,
                    EntryDraft::new(name.clone(), kind)
                        .parent(parent.clone())
                        .exported(!name.starts_with('_')),
                ));
            }
        }
        "import_statement" | "import_from_statement" | "future_import_statement" => {
            let name = signature(contents, node)
                .trim_start_matches("import ")
                .trim_start_matches("from ")
                .to_string();
            entries.push(entry(
                file,
                contents,
                node,
                EntryDraft::new(name, "import")
                    .parent(parent.clone())
                    .imported(true),
            ));
        }
        _ => {}
    }

    for child in named_children(node) {
        walk_python(file, contents, child, next_parent.clone(), entries);
    }
}

fn walk_markdown(file: &str, contents: &str, node: Node<'_>, entries: &mut Vec<OutlineEntry>) {
    match node.kind() {
        "atx_heading" | "setext_heading" => {
            let name = node_child_text(contents, node, "heading_content")
                .unwrap_or_else(|| markdown_heading_name(&signature(contents, node)));
            entries.push(entry(
                file,
                contents,
                node,
                EntryDraft::new(name, "module").exported(true),
            ));
        }
        _ => {}
    }
    for child in named_children(node) {
        walk_markdown(file, contents, child, entries);
    }
}

struct EntryDraft {
    name: String,
    outline_kind: String,
    parent: Option<String>,
    members: Vec<String>,
    visibility: Option<String>,
    exported: bool,
    imported: bool,
}

impl EntryDraft {
    fn new(name: String, outline_kind: impl Into<String>) -> Self {
        Self {
            name,
            outline_kind: outline_kind.into(),
            parent: None,
            members: Vec::new(),
            visibility: None,
            exported: false,
            imported: false,
        }
    }

    fn parent(mut self, parent: Option<String>) -> Self {
        self.parent = parent;
        self
    }

    fn members(mut self, members: Vec<String>) -> Self {
        self.members = members;
        self
    }

    fn visibility(mut self, visibility: Option<String>) -> Self {
        self.visibility = visibility;
        self
    }

    fn exported(mut self, exported: bool) -> Self {
        self.exported = exported;
        self
    }

    fn imported(mut self, imported: bool) -> Self {
        self.imported = imported;
        self
    }
}

fn entry(file: &str, contents: &str, node: Node<'_>, draft: EntryDraft) -> OutlineEntry {
    OutlineEntry {
        file: file.to_string(),
        range: OutlineRange {
            start_line: node.start_position().row as u64 + 1,
            end_line: node.end_position().row as u64 + 1,
            start_byte: node.start_byte(),
            end_byte: node.end_byte(),
        },
        name: draft.name,
        outline_kind: draft.outline_kind,
        signature: signature(contents, node),
        parent: draft.parent,
        members: draft.members,
        visibility: draft.visibility,
        exported: draft.exported,
        imported: draft.imported,
    }
}

fn named_children(node: Node<'_>) -> Vec<Node<'_>> {
    let mut cursor = node.walk();
    node.named_children(&mut cursor).collect()
}

fn node_name(contents: &str, node: Node<'_>) -> Option<String> {
    node_child_text(contents, node, "name").map(|name| clean_identifier(&name))
}

fn node_child_text(contents: &str, node: Node<'_>, field: &str) -> Option<String> {
    let child = node.child_by_field_name(field)?;
    Some(clean_identifier(&node_text(contents, child)?))
}

fn node_text(contents: &str, node: Node<'_>) -> Option<String> {
    node.utf8_text(contents.as_bytes()).ok().map(str::to_string)
}

fn clean_identifier(value: &str) -> String {
    value
        .trim()
        .trim_matches('{')
        .trim_matches('}')
        .trim_matches(';')
        .trim_matches(',')
        .trim()
        .to_string()
}

fn signature(contents: &str, node: Node<'_>) -> String {
    let text = node_text(contents, node).unwrap_or_default();
    let mut first = text.lines().next().unwrap_or("").trim().to_string();
    if first.is_empty() {
        first = text.trim().to_string();
    }
    if first.len() > 180 {
        first.truncate(177);
        first.push_str("...");
    }
    first
}

fn rust_visibility(contents: &str, node: Node<'_>) -> Option<String> {
    named_children(node)
        .into_iter()
        .find(|child| child.kind() == "visibility_modifier")
        .and_then(|child| node_text(contents, child))
        .map(|value| value.trim().to_string())
}

fn rust_exported(contents: &str, node: Node<'_>) -> bool {
    rust_visibility(contents, node)
        .as_deref()
        .is_some_and(|visibility| visibility.starts_with("pub"))
}

fn rust_members(contents: &str, node: Node<'_>) -> Vec<String> {
    rust_named_children(contents, node, "field_declaration")
}

fn rust_named_children(contents: &str, node: Node<'_>, kind: &str) -> Vec<String> {
    descendants(node)
        .into_iter()
        .filter(|child| child.kind() == kind)
        .filter_map(|child| {
            node_name(contents, child).or_else(|| first_named_identifier(contents, child))
        })
        .collect()
}

fn js_named_children(contents: &str, node: Node<'_>, kind: &str) -> Vec<String> {
    descendants(node)
        .into_iter()
        .filter(|child| child.kind() == kind)
        .filter_map(|child| {
            node_name(contents, child).or_else(|| first_named_identifier(contents, child))
        })
        .collect()
}

fn python_named_children(contents: &str, node: Node<'_>, kind: &str) -> Vec<String> {
    descendants(node)
        .into_iter()
        .filter(|child| child.kind() == kind)
        .filter_map(|child| node_name(contents, child))
        .collect()
}

fn descendants(node: Node<'_>) -> Vec<Node<'_>> {
    let mut found = Vec::new();
    let mut stack = named_children(node);
    while let Some(child) = stack.pop() {
        found.push(child);
        stack.extend(named_children(child));
    }
    found
}

fn first_named_identifier(contents: &str, node: Node<'_>) -> Option<String> {
    descendants(node)
        .into_iter()
        .find(|child| {
            matches!(
                child.kind(),
                "identifier" | "type_identifier" | "property_identifier" | "field_identifier"
            )
        })
        .and_then(|child| node_text(contents, child))
        .map(|value| clean_identifier(&value))
}

fn js_visibility(exported: bool) -> Option<String> {
    exported.then(|| "export".to_string())
}

fn markdown_heading_name(signature: &str) -> String {
    signature.trim_start_matches('#').trim().to_string()
}
