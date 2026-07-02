use std::path::PathBuf;

/// Resolves a path against the current working directory or a provided content root path.
///
/// The `path` argument can be:
/// - A file name (for example, `settings.yaml`).
/// - A relative path (for example, `config/settings.yaml`).
/// - An absolute path (for example, `/tmp/settings.yaml`).
///
/// File names and relative paths are resolved against the current working directory.
pub fn resolve_path(content_root_path: Option<&str>, path: &str) -> PathBuf {
    let path = PathBuf::from(path);

    if path.has_root() {
        return path;
    }

    let content_root_path = if let Some(content_root_path) = content_root_path {
        PathBuf::from(content_root_path)
    } else {
        std::env::current_dir().unwrap()
    };

    content_root_path.join(path)
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    #[test]
    fn test_return_absolute_path_ignoring_content_root_path() {
        let absolute_path = PathBuf::from("/tmp/settings.yaml");

        let resolved_path = super::resolve_path(
            Some("/ignored/content/root"),
            absolute_path.to_str().unwrap(),
        );

        assert_eq!(resolved_path, absolute_path);
    }

    #[test]
    fn test_resolve_relative_path_using_content_root_path() {
        let expected_path = PathBuf::from("/etc/wirio/config/settings.yaml");

        let resolved_path = super::resolve_path(Some("/etc/wirio"), "config/settings.yaml");

        assert_eq!(resolved_path, expected_path);
    }

    #[test]
    fn test_resolve_file_name_using_content_root_path() {
        let expected_path = PathBuf::from("/etc/wirio/settings.yaml");

        let resolved_path = super::resolve_path(Some("/etc/wirio"), "settings.yaml");

        assert_eq!(resolved_path, expected_path);
    }

    #[test]
    fn test_resolve_relative_path_using_current_directory_when_content_root_is_not_provided() {
        let relative_path = "config/settings.yaml";
        let current_directory = std::env::current_dir().unwrap();
        let expected_path = current_directory.join(relative_path);

        let resolved_path = super::resolve_path(None, "config/settings.yaml");

        assert_eq!(resolved_path, expected_path);
    }
}
