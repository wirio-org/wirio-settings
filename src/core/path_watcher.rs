use std::future::Future;
use std::path::{Path, PathBuf};
use std::time::Duration;

use notify_debouncer_mini::Debouncer;
use notify_debouncer_mini::notify::{RecommendedWatcher, RecursiveMode};
use notify_debouncer_mini::{DebouncedEvent, new_debouncer};
use tokio_util::sync::CancellationToken;

#[cfg(windows)]
const WINDOWS_HIDDEN_FILE_ATTRIBUTE: u32 = 0x0002;

#[cfg(windows)]
const WINDOWS_SYSTEM_FILE_ATTRIBUTE: u32 = 0x0004;

pub struct PathWatcher {
    path: PathBuf,
    cancellation_token: Option<CancellationToken>,
    debouncer: Option<Debouncer<RecommendedWatcher>>,
}

impl PathWatcher {
    pub fn new(path: PathBuf) -> Self {
        Self {
            path,
            cancellation_token: None,
            debouncer: None,
        }
    }

    pub fn watch<Callback, CallbackFuture>(
        &mut self,
        on_change: Callback,
    ) -> notify_debouncer_mini::notify::Result<()>
    where
        Callback: Fn() -> CallbackFuture + Send + 'static,
        CallbackFuture: Future<Output = ()> + Send + 'static,
    {
        let cancellation_token = CancellationToken::new();
        let task_cancellation_token = cancellation_token.clone();
        let (channel_sender, mut channel_receiver) = tokio::sync::mpsc::unbounded_channel();
        let mut debouncer = new_debouncer(Duration::from_millis(500), move |result| {
            let _ = channel_sender.send(result);
        })?;
        debouncer
            .watcher()
            .watch(&self.path, RecursiveMode::NonRecursive)?;
        let runtime = pyo3_async_runtimes::tokio::get_runtime();

        runtime.spawn(async move {
            loop {
                tokio::select! {
                    () = task_cancellation_token.cancelled() => break,
                    result = channel_receiver.recv() => {
                        let events = match result {
                            Some(Ok(events)) => events,
                            Some(Err(_)) => continue,
                            None => break,
                        };

                        if Self::are_relevant_changes(&events).await {
                            on_change().await;
                        }
                    }
                }
            }
        });

        self.cancellation_token.replace(cancellation_token);
        self.debouncer.replace(debouncer);
        Ok(())
    }

    async fn are_relevant_changes(events: &[DebouncedEvent]) -> bool {
        for event in events {
            if !Self::should_ignore_path(&event.path).await {
                return true;
            }
        }

        false
    }

    async fn should_ignore_path(path: &Path) -> bool {
        Self::is_dot_prefixed_path(path)
            || Self::is_hidden_path(path).await
            || Self::is_system_path(path).await
    }

    fn is_dot_prefixed_path(path: &Path) -> bool {
        path.file_name()
            .is_some_and(|file_name| file_name.as_encoded_bytes().starts_with(b"."))
    }

    #[cfg(windows)]
    async fn is_hidden_path(path: &Path) -> bool {
        Self::has_file_attribute(path, WINDOWS_HIDDEN_FILE_ATTRIBUTE).await
    }

    #[cfg(not(windows))]
    fn is_hidden_path(_path: &Path) -> std::future::Ready<bool> {
        std::future::ready(false)
    }

    #[cfg(windows)]
    async fn is_system_path(path: &Path) -> bool {
        Self::has_file_attribute(path, WINDOWS_SYSTEM_FILE_ATTRIBUTE).await
    }

    #[cfg(not(windows))]
    fn is_system_path(_path: &Path) -> std::future::Ready<bool> {
        std::future::ready(false)
    }

    #[cfg(windows)]
    async fn has_file_attribute(path: &Path, file_attribute: u32) -> bool {
        use std::os::windows::fs::MetadataExt;

        tokio::fs::symlink_metadata(path)
            .await
            .is_ok_and(|metadata| metadata.file_attributes() & file_attribute != 0)
    }
}

impl Drop for PathWatcher {
    fn drop(&mut self) {
        if let Some(cancellation_token) = &self.cancellation_token {
            cancellation_token.cancel();
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use notify_debouncer_mini::{DebouncedEvent, DebouncedEventKind};

    use super::PathWatcher;

    #[tokio::test]
    async fn test_ignore_dot_prefixed_path() {
        assert!(PathWatcher::is_dot_prefixed_path(Path::new(
            ".settings.yaml"
        )));
    }

    #[test]
    fn test_retain_regular_path() {
        assert!(!PathWatcher::is_dot_prefixed_path(Path::new(
            "settings.yaml"
        )));
    }

    #[cfg(not(windows))]
    #[tokio::test]
    async fn test_retain_path_when_checking_hidden_attribute() {
        assert!(!PathWatcher::is_hidden_path(Path::new("settings.yaml")).await);
    }

    #[cfg(not(windows))]
    #[tokio::test]
    async fn test_retain_path_when_checking_system_attribute() {
        assert!(!PathWatcher::is_system_path(Path::new("settings.yaml")).await);
    }

    #[tokio::test]
    async fn test_ignore_events_for_dot_prefixed_paths() {
        let events = [DebouncedEvent::new(
            PathBuf::from(".settings.yaml"),
            DebouncedEventKind::Any,
        )];

        assert!(!PathWatcher::are_relevant_changes(&events).await);
    }

    #[tokio::test]
    async fn test_retain_events_for_regular_paths() {
        let events = [DebouncedEvent::new(
            PathBuf::from("settings.yaml"),
            DebouncedEventKind::Any,
        )];

        assert!(PathWatcher::are_relevant_changes(&events).await);
    }
}
