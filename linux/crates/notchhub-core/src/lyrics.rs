//! Lyrics from LRCLIB (lrclib.net): free, no key. Only the title, artist and length of the song are sent.
use regex::Regex;
use serde::Deserialize;
use std::sync::OnceLock;

#[derive(Clone, Debug, PartialEq)]
pub struct LyricLine {
    pub time: f64,
    pub text: String,
}

#[derive(Clone, Debug, PartialEq, Default)]
pub struct Lyrics {
    /// Empty when the song only has plain text.
    pub synced: Vec<LyricLine>,
    pub plain: String,
    pub instrumental: bool,
}

#[derive(Debug, Deserialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct Hit {
    pub track_name: Option<String>,
    pub artist_name: Option<String>,
    pub duration: Option<f64>,
    pub plain_lyrics: Option<String>,
    pub synced_lyrics: Option<String>,
    pub instrumental: Option<bool>,
}

fn re(cell: &'static OnceLock<Regex>, pat: &str) -> &'static Regex {
    cell.get_or_init(|| Regex::new(pat).unwrap())
}

/// Removes the usual YouTube decorations: "(Official Video)", "[Lyrics]", "- Topic".
pub fn clean(s: &str) -> String {
    static DECOR: OnceLock<Regex> = OnceLock::new();
    static TOPIC: OnceLock<Regex> = OnceLock::new();
    let t = re(&DECOR, r"(?i)\s*[\(\[][^\)\]]*(official|video|audio|lyrics?|letra|visuali[sz]er|remaster|hd|4k)[^\)\]]*[\)\]]")
        .replace_all(s, "");
    re(&TOPIC, r"(?i)\s*-\s*Topic$").replace_all(&t, "").trim().to_string()
}

/// "[01:23.45] text" lines. Several timestamps on one line are allowed.
pub fn parse_lrc(s: &str) -> Vec<LyricLine> {
    static STAMP: OnceLock<Regex> = OnceLock::new();
    let stamp = re(&STAMP, r"\[(\d+):(\d+(?:\.\d+)?)\]");
    let mut out: Vec<LyricLine> = vec![];
    for line in s.lines() {
        let ms: Vec<_> = stamp.captures_iter(line).collect();
        let Some(last) = ms.last() else { continue };
        let end = last.get(0).unwrap().end();
        let text = line[end..].trim().to_string();
        for m in &ms {
            let (Ok(mins), Ok(secs)) = (m[1].parse::<f64>(), m[2].parse::<f64>()) else { continue };
            out.push(LyricLine { time: mins * 60.0 + secs, text: text.clone() });
        }
    }
    out.sort_by(|a, b| a.time.partial_cmp(&b.time).unwrap());
    out
}

/// Index of the line being sung at `position` seconds, if any started yet.
pub fn current_index(lines: &[LyricLine], position: f64) -> Option<usize> {
    let n = lines.partition_point(|l| l.time <= position);
    n.checked_sub(1)
}

fn to_lyrics(h: &Hit) -> Option<Lyrics> {
    if h.instrumental == Some(true) {
        return Some(Lyrics { instrumental: true, ..Default::default() });
    }
    let synced = h.synced_lyrics.as_deref().map(parse_lrc).unwrap_or_default();
    let plain = h.plain_lyrics.clone().unwrap_or_default();
    if synced.is_empty() && plain.is_empty() { None } else { Some(Lyrics { synced, plain, instrumental: false }) }
}

/// Picks the best search hit: synced lyrics first, then the closest length (within 5 s when the length is known).
pub fn choose_best(hits: &[Hit], duration: f64) -> Option<Lyrics> {
    let has = |s: &Option<String>| s.as_deref().map_or(false, |x| !x.is_empty());
    let usable: Vec<&Hit> = hits.iter().filter(|h| has(&h.synced_lyrics) || has(&h.plain_lyrics)).collect();
    let near: Vec<&Hit> =
        usable.iter().copied().filter(|h| duration <= 0.0 || (h.duration.unwrap_or(0.0) - duration).abs() <= 5.0).collect();
    let pool = if near.is_empty() { usable } else { near };
    let best = pool.into_iter().min_by(|a, b| {
        let ka = (if has(&a.synced_lyrics) { 0 } else { 1 }, (a.duration.unwrap_or(0.0) - duration).abs());
        let kb = (if has(&b.synced_lyrics) { 0 } else { 1 }, (b.duration.unwrap_or(0.0) - duration).abs());
        ka.partial_cmp(&kb).unwrap()
    })?;
    to_lyrics(best)
}

#[derive(Debug)]
pub enum FetchError {
    Offline,
}

async fn get(client: &reqwest::Client, path: &str, q: &[(&str, String)]) -> Option<(String, bool)> {
    let resp = client.get(format!("https://lrclib.net/api/{path}")).query(q).send().await.ok()?;
    let ok = resp.status().is_success();
    Some((resp.text().await.ok()?, ok))
}

/// `Ok(None)` means the song has no lyrics; `Err` means we could not reach the service.
pub async fn fetch(title: &str, artist: &str, duration: f64) -> Result<Option<Lyrics>, FetchError> {
    let client = reqwest::Client::builder()
        .user_agent("NotchHub/1.0")
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|_| FetchError::Offline)?;
    let (t, a) = (clean(title), clean(artist));
    let mut q = vec![("track_name", t.clone()), ("artist_name", a.clone())];
    if duration > 0.0 {
        q.push(("duration", (duration as i64).to_string()));
    }
    let (body, ok) = get(&client, "get", &q).await.ok_or(FetchError::Offline)?;
    if ok {
        if let Some(l) = serde_json::from_str::<Hit>(&body).ok().and_then(|h| to_lyrics(&h)) {
            return Ok(Some(l));
        }
    }
    let attempts = [vec![("track_name", t.clone()), ("artist_name", a.clone())], vec![("q", format!("{a} {t}"))]];
    for query in attempts {
        let Some((body, ok)) = get(&client, "search", &query).await else { continue };
        if !ok {
            continue;
        }
        if let Some(l) = serde_json::from_str::<Vec<Hit>>(&body).ok().and_then(|h| choose_best(&h, duration)) {
            return Ok(Some(l));
        }
    }
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_multiple_stamps_and_sorts() {
        let l = parse_lrc("[00:20.00] second\n[00:05.50][01:00.00]chorus\nno stamp\n[00:10]ten");
        let times: Vec<f64> = l.iter().map(|x| x.time).collect();
        assert_eq!(times, vec![5.5, 10.0, 20.0, 60.0]);
        assert_eq!(l[0].text, "chorus");
        assert_eq!(l[3].text, "chorus");
    }

    #[test]
    fn current_line_follows_position() {
        let l = parse_lrc("[00:05]a\n[00:10]b");
        assert_eq!(current_index(&l, 1.0), None);
        assert_eq!(current_index(&l, 5.0), Some(0));
        assert_eq!(current_index(&l, 9.99), Some(0));
        assert_eq!(current_index(&l, 100.0), Some(1));
    }

    #[test]
    fn cleans_youtube_decorations() {
        assert_eq!(clean("Song (Official Video) [HD]"), "Song");
        assert_eq!(clean("Artist - Topic"), "Artist");
        assert_eq!(clean("Song (feat. Someone)"), "Song (feat. Someone)");
    }

    #[test]
    fn prefers_synced_and_close_duration() {
        let plain = Hit { plain_lyrics: Some("words".into()), duration: Some(200.0), ..Default::default() };
        let synced_far = Hit { synced_lyrics: Some("[00:01]x".into()), duration: Some(260.0), ..Default::default() };
        let synced_near = Hit { synced_lyrics: Some("[00:01]near".into()), duration: Some(201.0), ..Default::default() };
        let l = choose_best(&[plain.clone(), synced_far, synced_near], 200.0).unwrap();
        assert_eq!(l.synced[0].text, "near");
        let l = choose_best(&[plain], 200.0).unwrap();
        assert!(l.synced.is_empty() && l.plain == "words");
    }

    #[test]
    fn instrumental_flag() {
        let h = Hit { instrumental: Some(true), ..Default::default() };
        assert!(to_lyrics(&h).unwrap().instrumental);
    }
}
