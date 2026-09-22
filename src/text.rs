//! Small string helpers ported from the baseline's `utl/` folder.

/// Options for [`plural`].
#[derive(Debug, Clone)]
pub struct PluralOptions {
    /// Appended to the root when plural. Default `"s"`.
    pub plural: &'static str,
    /// Appended to the root when singular. Default `""`.
    pub singular: &'static str,
}

impl Default for PluralOptions {
    fn default() -> Self {
        Self {
            plural: "s",
            singular: "",
        }
    }
}

/// Systematically pluralise a root word: `plural("file", 3)` → `"files"`,
/// `plural_with("compan", 1, PluralOptions { plural: "ies", singular: "y" })` → `"company"`.
pub fn plural(word: &str, count: usize) -> String {
    plural_with(word, count, PluralOptions::default())
}

/// [`plural`] with custom suffixes.
pub fn plural_with(word: &str, count: usize, options: PluralOptions) -> String {
    if count != 1 {
        format!("{word}{}", options.plural)
    } else {
        format!("{word}{}", options.singular)
    }
}

/// Upper-case the first character.
pub fn capitalize(value: &str) -> String {
    let mut chars = value.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}

/// `"dry-run"` / `"dry_run"` → `"Dry Run"`.
pub fn action_to_string(value: &str) -> String {
    value
        .split(['_', '-'])
        .filter(|part| !part.is_empty())
        .map(capitalize)
        .collect::<Vec<_>>()
        .join(" ")
}

/// Quote each non-empty value and join with `", "`; used in error messages that list valid options.
pub fn array_to_string_list<S: AsRef<str>>(values: &[S]) -> String {
    array_to_string_list_with(values, ", ")
}

/// [`array_to_string_list`] with a custom delimiter.
pub fn array_to_string_list_with<S: AsRef<str>>(values: &[S], delimiter: &str) -> String {
    values
        .iter()
        .map(|value| value.as_ref().trim())
        .filter(|value| !value.is_empty())
        .map(|value| format!("\"{value}\""))
        .collect::<Vec<_>>()
        .join(delimiter)
}

#[cfg(test)]
mod tests {
    use super::*;

    // Ported from utl/plural.test.ts
    #[test]
    fn plural_returns_expected_result_for_file() {
        assert_eq!(plural("file", 1), "file");
        assert_eq!(plural("file", 3), "files");
        assert_eq!(plural("file", 0), "files");
    }

    #[test]
    fn plural_returns_company_for_value_of_1() {
        let options = PluralOptions {
            plural: "ies",
            singular: "y",
        };
        assert_eq!(plural_with("compan", 1, options), "company");
    }

    #[test]
    fn plural_returns_companies_for_value_of_5() {
        let options = PluralOptions {
            plural: "ies",
            singular: "y",
        };
        assert_eq!(plural_with("compan", 5, options), "companies");
    }

    // Ported from utl/capitalize.test.ts (the "throws on non-string" case is a type error in Rust)
    #[test]
    fn capitalize_returns_expected_results() {
        assert_eq!(capitalize("foo"), "Foo");
        assert_eq!(capitalize("foo bar"), "Foo bar");
        assert_eq!(capitalize(""), "");
    }

    // Ported from utl/convertActionToString.test.ts
    #[test]
    fn action_to_string_returns_expected_results() {
        assert_eq!(action_to_string("dry-run"), "Dry Run");
        assert_eq!(action_to_string("dry_run"), "Dry Run");
        assert_eq!(action_to_string("run"), "Run");
    }

    // Ported from utl/arrayToStringList.test.ts
    #[test]
    fn array_to_string_list_returns_a_string_from_the_array() {
        assert_eq!(
            array_to_string_list(&["a", "b", "c"]),
            "\"a\", \"b\", \"c\""
        );
        assert_eq!(array_to_string_list(&["a", " ", "c"]), "\"a\", \"c\"");
    }
}
