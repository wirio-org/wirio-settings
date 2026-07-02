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
