/// Convert a title to a conservative ASCII slug.
pub fn slugify_ascii(title: &str) -> String {
    let mut slug = String::new();
    let mut previous_dash = false;

    for character in title.chars().flat_map(char::to_lowercase) {
        if character.is_ascii_alphanumeric() {
            slug.push(character);
            previous_dash = false;
        } else if !previous_dash && !slug.is_empty() {
            slug.push('-');
            previous_dash = true;
        }
    }

    slug.trim_matches('-').to_string()
}
