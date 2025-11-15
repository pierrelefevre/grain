use crate::state::User;

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum Action {
    Pull,
    Push,
    Delete,
}

impl Action {
    pub fn as_str(&self) -> &str {
        match self {
            Action::Pull => "pull",
            Action::Push => "push",
            Action::Delete => "delete",
        }
    }
}

/// Check if a user has permission to perform an action on a specific repository/tag
pub fn has_permission(user: &User, repository: &str, tag: Option<&str>, action: Action) -> bool {
    // If user has no permissions defined, deny by default
    if user.permissions.is_empty() {
        return false;
    }

    let action_str = action.as_str();

    for perm in &user.permissions {
        // Check if repository matches
        if !matches_pattern(&perm.repository, repository) {
            continue;
        }

        // Check if tag matches (if tag is required for the operation)
        if let Some(tag_name) = tag {
            if !matches_pattern(&perm.tag, tag_name) {
                continue;
            }
        }

        // Check if action is allowed
        if perm.actions.contains(&action_str.to_string()) {
            return true;
        }
    }

    false
}

/// Match a pattern with wildcards (* and ?)
fn matches_pattern(pattern: &str, value: &str) -> bool {
    if pattern == "*" {
        return true;
    }

    if pattern == value {
        return true;
    }

    // Simple wildcard matching
    if pattern.contains('*') {
        let parts: Vec<&str> = pattern.split('*').collect();

        if parts.len() == 2 {
            let prefix = parts[0];
            let suffix = parts[1];

            if prefix.is_empty() && suffix.is_empty() {
                return true; // "*"
            }

            if prefix.is_empty() {
                return value.ends_with(suffix);
            }

            if suffix.is_empty() {
                return value.starts_with(prefix);
            }

            return value.starts_with(prefix) && value.ends_with(suffix);
        }
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::Permission;

    #[test]
    fn test_pattern_matching() {
        assert!(matches_pattern("*", "anything"));
        assert!(matches_pattern("myorg/*", "myorg/myrepo"));
        assert!(matches_pattern("myorg/*", "myorg/another"));
        assert!(!matches_pattern("myorg/*", "other/repo"));
        assert!(matches_pattern("v*", "v1.0.0"));
        assert!(matches_pattern("*-prod", "app-prod"));
        assert!(matches_pattern("exact", "exact"));
        assert!(!matches_pattern("exact", "notexact"));
    }

    #[test]
    fn test_has_permission() {
        let user = User {
            username: "alice".to_string(),
            password: "pass".to_string(),
            permissions: vec![
                Permission {
                    repository: "myorg/myrepo".to_string(),
                    tag: "latest".to_string(),
                    actions: vec!["pull".to_string()],
                },
                Permission {
                    repository: "myorg/myrepo".to_string(),
                    tag: "dev".to_string(),
                    actions: vec!["pull".to_string(), "push".to_string()],
                },
            ],
        };

        assert!(has_permission(
            &user,
            "myorg/myrepo",
            Some("latest"),
            Action::Pull
        ));
        assert!(!has_permission(
            &user,
            "myorg/myrepo",
            Some("latest"),
            Action::Push
        ));
        assert!(has_permission(
            &user,
            "myorg/myrepo",
            Some("dev"),
            Action::Push
        ));
        assert!(!has_permission(
            &user,
            "other/repo",
            Some("latest"),
            Action::Pull
        ));
    }

    #[test]
    fn test_wildcard_permissions() {
        let admin = User {
            username: "admin".to_string(),
            password: "admin".to_string(),
            permissions: vec![Permission {
                repository: "*".to_string(),
                tag: "*".to_string(),
                actions: vec!["pull".to_string(), "push".to_string(), "delete".to_string()],
            }],
        };

        assert!(has_permission(
            &admin,
            "any/repo",
            Some("any-tag"),
            Action::Pull
        ));
        assert!(has_permission(
            &admin,
            "any/repo",
            Some("any-tag"),
            Action::Push
        ));
        assert!(has_permission(
            &admin,
            "any/repo",
            Some("any-tag"),
            Action::Delete
        ));
    }

    #[test]
    fn test_no_permissions_deny() {
        let user = User {
            username: "noperms".to_string(),
            password: "pass".to_string(),
            permissions: vec![],
        };

        assert!(!has_permission(
            &user,
            "any/repo",
            Some("tag"),
            Action::Pull
        ));
        assert!(!has_permission(
            &user,
            "any/repo",
            Some("tag"),
            Action::Push
        ));
    }

    #[test]
    fn test_repository_wildcard() {
        let user = User {
            username: "dev".to_string(),
            password: "pass".to_string(),
            permissions: vec![Permission {
                repository: "myorg/*".to_string(),
                tag: "*".to_string(),
                actions: vec!["pull".to_string()],
            }],
        };

        assert!(has_permission(
            &user,
            "myorg/repo1",
            Some("latest"),
            Action::Pull
        ));
        assert!(has_permission(
            &user,
            "myorg/repo2",
            Some("v1.0"),
            Action::Pull
        ));
        assert!(!has_permission(
            &user,
            "other/repo",
            Some("latest"),
            Action::Pull
        ));
    }

    #[test]
    fn test_tag_wildcard() {
        let user = User {
            username: "dev".to_string(),
            password: "pass".to_string(),
            permissions: vec![Permission {
                repository: "myorg/myrepo".to_string(),
                tag: "v*".to_string(),
                actions: vec!["pull".to_string()],
            }],
        };

        assert!(has_permission(
            &user,
            "myorg/myrepo",
            Some("v1.0.0"),
            Action::Pull
        ));
        assert!(has_permission(
            &user,
            "myorg/myrepo",
            Some("v2.0.0"),
            Action::Pull
        ));
        assert!(!has_permission(
            &user,
            "myorg/myrepo",
            Some("latest"),
            Action::Pull
        ));
    }
}
