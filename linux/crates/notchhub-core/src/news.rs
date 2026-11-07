//! News categories and sources. Mirrors the macOS app: a few defaults the user can delete (and bring back),
//! plus anything the user adds. A category either has its own sources or is a keyword-filtered view of another.
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct NewsCategory {
    pub id: String,
    pub name: String,
    /// Filtered view of another category (the built-in "IA" tabs).
    #[serde(default)]
    pub parent: Option<String>,
    /// "ai" or "ai_market".
    #[serde(default)]
    pub filter: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Feed {
    pub name: String,
    pub url: String,
    pub category: String,
    #[serde(default)]
    pub only_links_containing: Option<String>,
    #[serde(default = "yes")]
    pub enabled: bool,
}
fn yes() -> bool {
    true
}

impl Feed {
    pub fn new(name: &str, url: &str, category: &str) -> Self {
        Feed { name: name.into(), url: url.into(), category: category.into(), only_links_containing: None, enabled: true }
    }
}

pub fn default_categories() -> Vec<NewsCategory> {
    let c = |id: &str, name: &str, parent: Option<&str>, filter: Option<&str>| NewsCategory {
        id: id.into(),
        name: name.into(),
        parent: parent.map(Into::into),
        filter: filter.map(Into::into),
    };
    vec![
        c("tech", "Tecnología", None, None),
        c("ai", "IA", Some("tech"), Some("ai")),
        c("market", "Bolsa", None, None),
        c("ai_market", "Bolsa e IA", Some("market"), Some("ai_market")),
    ]
}

pub fn default_feeds() -> Vec<Feed> {
    let mut cinco = Feed::new(
        "Cinco Días",
        "https://feeds.elpais.com/mrss-s/pages/ep/site/cincodias.elpais.com/portada",
        "market",
    );
    cinco.only_links_containing = Some("/mercados-financieros/".into());
    vec![
        Feed::new("Xataka", "https://www.xataka.com/feedburner.xml", "tech"),
        Feed::new("Genbeta", "https://www.genbeta.com/feedburner.xml", "tech"),
        Feed::new("Hipertextual", "https://hipertextual.com/feed", "tech"),
        Feed::new(
            "El País Tecnología",
            "https://feeds.elpais.com/mrss-s/pages/ep/site/elpais.com/section/tecnologia/portada",
            "tech",
        ),
        Feed::new("Expansión", "https://e00-expansion.uecdn.es/rss/mercados.xml", "market"),
        cinco,
        Feed::new("Investing.com", "https://es.investing.com/rss/news_25.rss", "market"),
    ]
}

fn default_ids() -> Vec<String> {
    default_categories().into_iter().map(|c| c.id).chain(default_feeds().into_iter().map(|f| f.url)).collect()
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct NewsConfig {
    pub categories: Vec<NewsCategory>,
    pub feeds: Vec<Feed>,
    /// Defaults this config has already seen. A default that is not in the lists but is in here was deleted by the user.
    pub seen_defaults: Vec<String>,
}

impl Default for NewsConfig {
    fn default() -> Self {
        NewsConfig { categories: default_categories(), feeds: default_feeds(), seen_defaults: default_ids() }
    }
}

impl NewsConfig {
    /// Adds defaults that a newer version brought (never seen before) and drops feeds without a category.
    pub fn normalize(&mut self) {
        for c in default_categories() {
            if !self.seen_defaults.contains(&c.id) && !self.categories.iter().any(|x| x.id == c.id) {
                self.categories.push(c);
            }
        }
        for f in default_feeds() {
            if !self.seen_defaults.contains(&f.url) && !self.feeds.iter().any(|x| x.url == f.url) {
                self.feeds.push(f);
            }
        }
        for id in default_ids() {
            if !self.seen_defaults.contains(&id) {
                self.seen_defaults.push(id);
            }
        }
        let cats: Vec<String> = self.categories.iter().map(|c| c.id.clone()).collect();
        self.feeds.retain(|f| cats.contains(&f.category));
    }

    /// Brings back the default categories and sources that were deleted. What the user added stays.
    pub fn restore_defaults(&mut self) {
        for c in default_categories() {
            if !self.categories.iter().any(|x| x.id == c.id) {
                self.categories.push(c);
            }
        }
        for f in default_feeds() {
            if !self.feeds.iter().any(|x| x.url == f.url) {
                self.feeds.push(f);
            }
        }
    }

    pub fn add_category(&mut self, name: &str) -> String {
        let id = format!("c{}", chrono::Utc::now().timestamp_micros());
        self.categories.push(NewsCategory { id: id.clone(), name: name.trim().into(), parent: None, filter: None });
        id
    }

    /// Deleting a category deletes its sources and the filtered views that depend on it.
    pub fn delete_category(&mut self, id: &str) {
        self.feeds.retain(|f| f.category != id);
        self.categories.retain(|c| c.id != id && c.parent.as_deref() != Some(id));
        let cats: Vec<String> = self.categories.iter().map(|c| c.id.clone()).collect();
        self.feeds.retain(|f| cats.contains(&f.category));
    }

    /// Adds a source; returns the name of the category that already has it, if it is a duplicate.
    pub fn add_feed(&mut self, feed: Feed) -> Result<(), String> {
        if let Some(other) = self.feeds.iter().find(|f| f.url == feed.url) {
            let name = self.categories.iter().find(|c| c.id == other.category).map(|c| c.name.clone()).unwrap_or_default();
            return Err(name);
        }
        self.feeds.push(feed);
        Ok(())
    }

    pub fn feeds_in(&self, category: &str) -> impl Iterator<Item = &Feed> {
        let category = category.to_string();
        self.feeds.iter().filter(move |f| f.category == category)
    }

    /// Tabs to show: a filtered view disappears when the category it filters is gone.
    pub fn visible_categories(&self) -> Vec<&NewsCategory> {
        self.categories
            .iter()
            .filter(|c| c.parent.as_ref().map_or(true, |p| self.categories.iter().any(|x| &x.id == p)))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fresh_config_has_all_defaults() {
        let c = NewsConfig::default();
        assert_eq!(c.categories.len(), 4);
        assert_eq!(c.feeds.len(), 7);
    }

    #[test]
    fn deleted_default_does_not_come_back() {
        let mut c = NewsConfig::default();
        c.feeds.retain(|f| f.name != "Xataka");
        c.delete_category("market");
        c.normalize();
        assert!(!c.feeds.iter().any(|f| f.name == "Xataka"));
        assert!(!c.categories.iter().any(|x| x.id == "market" || x.id == "ai_market"));
        assert!(!c.feeds.iter().any(|f| f.category == "market"));
    }

    #[test]
    fn restore_brings_defaults_back_and_keeps_custom() {
        let mut c = NewsConfig::default();
        let id = c.add_category("Coches");
        c.add_feed(Feed::new("Motor", "https://motor.example/feed", &id)).unwrap();
        c.delete_category("tech");
        c.restore_defaults();
        assert!(c.categories.iter().any(|x| x.id == "tech"));
        assert!(c.feeds.iter().any(|f| f.name == "Motor"));
        assert!(c.feeds.iter().any(|f| f.name == "Xataka"));
    }

    #[test]
    fn deleting_parent_removes_filtered_view() {
        let mut c = NewsConfig::default();
        c.delete_category("tech");
        assert!(!c.categories.iter().any(|x| x.id == "ai"));
    }

    #[test]
    fn duplicate_feed_reports_category() {
        let mut c = NewsConfig::default();
        let err = c.add_feed(Feed::new("X", "https://www.xataka.com/feedburner.xml", "market")).unwrap_err();
        assert_eq!(err, "Tecnología");
    }

    #[test]
    fn old_config_without_seen_list_gets_new_defaults_once() {
        let mut c = NewsConfig { categories: vec![], feeds: vec![], seen_defaults: vec![] };
        c.normalize();
        assert_eq!(c.categories.len(), 4);
        c.categories.clear();
        c.normalize();
        assert!(c.categories.is_empty());
    }
}
