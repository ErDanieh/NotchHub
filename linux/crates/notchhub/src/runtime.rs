//! One tokio runtime for everything that waits: network, D-Bus, Docker.
use std::sync::OnceLock;
use tokio::runtime::{Builder, Runtime};

pub fn rt() -> &'static Runtime {
    static RT: OnceLock<Runtime> = OnceLock::new();
    RT.get_or_init(|| Builder::new_multi_thread().worker_threads(2).enable_all().thread_name("notchhub-io").build().expect("tokio runtime"))
}

/// Runs a future on the runtime and waits for it from the GTK main loop.
pub async fn on_rt<T: Send + 'static>(fut: impl std::future::Future<Output = T> + Send + 'static) -> Option<T> {
    rt().spawn(fut).await.ok()
}
