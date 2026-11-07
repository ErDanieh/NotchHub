//! Reading news feeds: RSS and Atom parsing, finding the feed of a website, and the keyword views ("IA").
use crate::news::{Feed, NewsConfig};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::OnceLock;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct NewsItem {
    pub title: String,
    pub link: String,
    pub source: String,
    /// Unix seconds.
    pub date: i64,
    pub image: Option<String>,
}

const AI_WORDS: &str = "inteligencia artificial|ia generativa|openai|chatgpt|anthropic|claude|gemini|deepseek|copilot|mistral|grok|llm|gpt-?\\d|modelos? de lenguaje|machine learning|agentes? de ia";
const MARKET_WORDS: &str = "nvidia|amd|tsmc|broadcom|palantir|semiconductores|chips?|centros? de datos|alphabet|microsoft|meta platforms|oracle|burbuja";

fn ai_regex() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(&format!(r"(?i)\b(?:{AI_WORDS})\b|\bIA\b")).unwrap())
}
fn ai_market_regex() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(&format!(r"(?i)\b(?:{AI_WORDS}|{MARKET_WORDS})\b|\bIA\b")).unwrap())
}

/// "IA" is matched in capitals only (so it does not match the Spanish word "ia"); the rest ignores case.
pub fn matches_filter(kind: &str, title: &str) -> bool {
    let re = if kind == "ai_market" { ai_market_regex() } else { ai_regex() };
    // the regex above is case-insensitive for everything, so re-check the bare "ia" case
    re.find_iter(title).any(|m| {
        let t = m.as_str();
        !t.eq_ignore_ascii_case("ia") || t == "IA"
    })
}

fn img_regex() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r#"(?i)<img[^>]+src=["']([^"']+)["']"#).unwrap())
}

fn first_image(html: &str) -> Option<String> {
    img_regex().captures_iter(html).map(|c| c[1].to_string()).find(|u| u.starts_with("http") && !u.contains("pixel") && !u.contains("imrworldwide")).map(|u| u.replacen("http://", "https://", 1))
}

fn clean(s: &str) -> String {
    s.replace("&nbsp;", " ").replace("&amp;", "&").replace("&quot;", "\"").replace("&#8217;", "’").trim().to_string()
}

/// Parses an RSS or Atom document. `now` is used for items without a date.
pub fn parse_items(data: &[u8], source: &str, now: i64) -> Vec<NewsItem> {
    let Ok(feed) = feed_rs::parser::parse(data) else { return vec![] };
    feed.entries
        .into_iter()
        .filter_map(|e| {
            let title = clean(&e.title?.content);
            let link = e.links.first()?.href.clone();
            if title.is_empty() || !link.starts_with("http") {
                return None;
            }
            let date = e.published.or(e.updated).map_or(now, |d| d.timestamp());
            let image = e
                .media
                .iter()
                .flat_map(|m| m.thumbnails.iter().map(|t| t.image.uri.clone()).chain(m.content.iter().filter(|c| c.content_type.as_ref().map_or(false, |t| t.ty() == "image")).filter_map(|c| c.url.as_ref().map(|u| u.to_string()))))
                .next()
                .or_else(|| e.summary.as_ref().and_then(|s| first_image(&s.content)))
                .or_else(|| e.content.as_ref().and_then(|c| c.body.as_deref()).and_then(first_image));
            Some(NewsItem { title, link, source: source.to_string(), date, image })
        })
        .collect()
}

/// Feed addresses a web page announces or links: `<link rel="alternate" type="application/rss+xml">` first,
/// then ordinary links that look like feeds (pages that list their RSS, such as "rss.html").
pub fn extract_candidates(html: &str, base: &url::Url) -> Vec<String> {
    let doc = scraper::Html::parse_document(html);
    let mut out: Vec<String> = vec![];
    let mut add = |href: &str| {
        if let Ok(u) = base.join(&href.replace("&amp;", "&")) {
            let s = u.to_string();
            if !out.contains(&s) {
                out.push(s);
            }
        }
    };
    if let Ok(sel) = scraper::Selector::parse("link[rel~=alternate]") {
        for el in doc.select(&sel) {
            let t = el.value().attr("type").unwrap_or("").to_lowercase();
            if t.contains("rss") || t.contains("atom") {
                if let Some(h) = el.value().attr("href") {
                    add(h);
                }
            }
        }
    }
    if let Ok(sel) = scraper::Selector::parse("a[href]") {
        let mut n = 0;
        for el in doc.select(&sel) {
            let h = el.value().attr("href").unwrap_or("");
            let low = h.to_lowercase();
            if (low.contains("rss") || low.contains("feed") || low.ends_with(".xml")) && !low.ends_with(".html") && !low.ends_with(".htm") {
                add(h);
                n += 1;
                if n >= 12 {
                    break;
                }
            }
        }
    }
    out
}

fn common_paths(base: &url::Url) -> Vec<String> {
    ["/feed", "/rss", "/rss.xml", "/feed.xml", "/rss/portada.xml", "/index.xml"]
        .iter()
        .filter_map(|p| base.join(p).ok().map(|u| u.to_string()))
        .collect()
}

pub async fn fetch_feed(client: &reqwest::Client, feed: &Feed, now: i64) -> Option<Vec<NewsItem>> {
    let resp = client.get(&feed.url).send().await.ok()?;
    if !resp.status().is_success() {
        return None;
    }
    let data = resp.bytes().await.ok()?;
    let mut items = parse_items(&data, &feed.name, now);
    if let Some(key) = &feed.only_links_containing {
        items.retain(|i| i.link.contains(key.as_str())); // the front page also carries non-market stories
    }
    Some(items)
}

/// Takes the address of a website or of a feed and returns a working feed with its items.
pub async fn discover(client: &reqwest::Client, address: &str, category: &str, name: &str, now: i64) -> Option<(Feed, Vec<NewsItem>)> {
    let attempt = |url: String| async move {
        let f = Feed::new(name, &url, category);
        match fetch_feed(client, &f, now).await {
            Some(items) if !items.is_empty() => Some((f, items)),
            _ => None,
        }
    };
    if let Some(ok) = attempt(address.to_string()).await {
        return Some(ok);
    }
    let base = url::Url::parse(address).ok()?;
    let mut candidates = vec![];
    if let Ok(r) = client.get(address).send().await {
        if let Ok(body) = r.text().await {
            candidates = extract_candidates(&body.chars().take(400_000).collect::<String>(), &base);
        }
    }
    candidates.extend(common_paths(&base));
    for c in candidates {
        if let Some(ok) = attempt(c).await {
            return Some(ok);
        }
    }
    None
}

/// Stories of every category from what was fetched per feed address.
pub fn build_view(config: &NewsConfig, by_feed: &HashMap<String, Vec<NewsItem>>) -> HashMap<String, Vec<NewsItem>> {
    let all_of = |category: &str| -> Vec<NewsItem> {
        let mut v: Vec<NewsItem> = config
            .feeds
            .iter()
            .filter(|f| f.enabled && f.category == category)
            .flat_map(|f| by_feed.get(&f.url).cloned().unwrap_or_default())
            .collect();
        v.sort_by(|a, b| b.date.cmp(&a.date));
        let mut seen = std::collections::HashSet::new();
        v.retain(|i| seen.insert(i.link.clone()));
        v
    };
    let mut out = HashMap::new();
    for c in &config.categories {
        let mut v = match (&c.parent, &c.filter) {
            (Some(p), Some(kind)) => all_of(p).into_iter().filter(|i| matches_filter(kind, &i.title)).collect(),
            (Some(p), None) => all_of(p),
            _ => all_of(&c.id),
        };
        v.truncate(40);
        out.insert(c.id.clone(), v);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const RSS: &str = r#"<?xml version="1.0"?><rss version="2.0" xmlns:media="http://search.yahoo.com/mrss/"><channel><title>T</title>
<item><title>OpenAI lanza un modelo nuevo</title><link>https://x.example/a</link><pubDate>Sat, 07 Nov 2026 10:00:00 +0000</pubDate>
<media:thumbnail url="https://x.example/a.jpg"/></item>
<item><title>Receta de tortilla</title><link>https://x.example/b</link><pubDate>Sat, 07 Nov 2026 09:00:00 +0000</pubDate>
<description>&lt;p&gt;&lt;img src="http://x.example/b.png"/&gt;&lt;/p&gt;</description></item>
<item><title></title><link>https://x.example/empty</link></item></channel></rss>"#;

    #[test]
    fn parses_rss_with_images_and_skips_empty() {
        let v = parse_items(RSS.as_bytes(), "Test", 0);
        assert_eq!(v.len(), 2);
        assert_eq!(v[0].image.as_deref(), Some("https://x.example/a.jpg"));
        assert_eq!(v[1].image.as_deref(), Some("https://x.example/b.png"));
        assert!(v[0].date > v[1].date);
    }

    #[test]
    fn parses_atom() {
        let atom = r#"<?xml version="1.0"?><feed xmlns="http://www.w3.org/2005/Atom"><title>A</title><updated>2026-01-01T00:00:00Z</updated>
<entry><title>Hola</title><link href="https://a.example/1"/><id>1</id><updated>2026-01-01T00:00:00Z</updated></entry></feed>"#;
        let v = parse_items(atom.as_bytes(), "A", 0);
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].link, "https://a.example/1");
    }

    #[test]
    fn finds_feeds_announced_in_html_and_listed_in_links() {
        let base = url::Url::parse("https://www.informacion.es/rss.html").unwrap();
        let html = r#"<html><head><link rel="alternate" type="application/rss+xml" href="/feed/portada.xml"></head>
<body><a href="/rss/alicante.xml">Alicante</a><a href="/rss.html">RSS</a><a href="/contacto">x</a></body></html>"#;
        let c = extract_candidates(html, &base);
        assert_eq!(c[0], "https://www.informacion.es/feed/portada.xml");
        assert!(c.contains(&"https://www.informacion.es/rss/alicante.xml".to_string()));
        assert!(!c.iter().any(|u| u.ends_with("rss.html") || u.ends_with("contacto")));
    }

    #[test]
    fn ai_filter_matches_words_and_capital_ia_only() {
        assert!(matches_filter("ai", "OpenAI presenta GPT-5"));
        assert!(matches_filter("ai", "La IA cambia el trabajo"));
        assert!(!matches_filter("ai", "Hay una ia en la lista de palabras sueltas"));
        assert!(!matches_filter("ai", "Receta de tortilla"));
        assert!(matches_filter("ai_market", "Nvidia sube en bolsa"));
        assert!(!matches_filter("ai", "Nvidia sube en bolsa"));
    }

    #[test]
    fn view_filters_by_category_and_skips_disabled_sources() {
        let mut cfg = NewsConfig::default();
        let item = |t: &str, l: &str, d: i64| NewsItem { title: t.into(), link: l.into(), source: "s".into(), date: d, image: None };
        let mut by_feed = HashMap::new();
        by_feed.insert("https://www.xataka.com/feedburner.xml".to_string(), vec![item("OpenAI nuevo", "l1", 2), item("Móvil nuevo", "l2", 3)]);
        by_feed.insert("https://hipertextual.com/feed".to_string(), vec![item("Otro", "l3", 1)]);
        let v = build_view(&cfg, &by_feed);
        assert_eq!(v["tech"].len(), 3);
        assert_eq!(v["tech"][0].link, "l2");
        assert_eq!(v["ai"].len(), 1);
        cfg.feeds.iter_mut().find(|f| f.name == "Hipertextual").unwrap().enabled = false;
        assert_eq!(build_view(&cfg, &by_feed)["tech"].len(), 2);
    }
}
