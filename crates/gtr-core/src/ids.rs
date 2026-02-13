/// Generate a unique work item ID with format: `wi-{nanoid}`
pub fn work_item_id() -> String {
    format!("wi-{}", nanoid::nanoid!())
}

/// Generate a unique convoy ID with format: `cv-{nanoid}`
pub fn convoy_id() -> String {
    format!("cv-{}", nanoid::nanoid!())
}

/// Generate a unique agent ID preserving the given name, format: `{name}-{nanoid}`
pub fn agent_id(name: &str) -> String {
    format!("{}-{}", name, nanoid::nanoid!())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn work_item_id_has_correct_format() {
        let id = work_item_id();
        assert!(id.starts_with("wi-"), "expected 'wi-' prefix, got: {id}");
        let suffix = &id[3..];
        assert!(!suffix.is_empty(), "expected nanoid suffix after prefix");
        assert!(
            suffix.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-'),
            "suffix contains invalid characters: {suffix}"
        );
    }

    #[test]
    fn convoy_id_has_correct_format() {
        let id = convoy_id();
        assert!(id.starts_with("cv-"), "expected 'cv-' prefix, got: {id}");
        let suffix = &id[3..];
        assert!(!suffix.is_empty(), "expected nanoid suffix after prefix");
        assert!(
            suffix.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-'),
            "suffix contains invalid characters: {suffix}"
        );
    }

    #[test]
    fn agent_id_preserves_name() {
        let id = agent_id("rictus");
        assert!(
            id.starts_with("rictus-"),
            "expected 'rictus-' prefix, got: {id}"
        );
        let suffix = &id["rictus-".len()..];
        assert!(!suffix.is_empty(), "expected nanoid suffix after name");
    }

    #[test]
    fn generated_ids_are_unique() {
        let mut ids = HashSet::new();
        for _ in 0..100 {
            ids.insert(work_item_id());
            ids.insert(convoy_id());
            ids.insert(agent_id("test"));
        }
        assert_eq!(ids.len(), 300, "expected 300 unique IDs, got {}", ids.len());
    }
}
