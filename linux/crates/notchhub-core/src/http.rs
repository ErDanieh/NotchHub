//! Shared HTTP client settings.
pub use reqwest::Client;

pub fn client() -> Option<Client> {
    Client::builder().user_agent("Mozilla/5.0 (X11; Linux) NotchHub/1.0").timeout(std::time::Duration::from_secs(12)).build().ok()
}
