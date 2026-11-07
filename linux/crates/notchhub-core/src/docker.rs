//! Docker containers through the `docker` command (the same one the user runs in a terminal).
use serde::Deserialize;
use std::process::Command;

#[derive(Clone, Debug, PartialEq)]
pub struct Container {
    pub id: String,
    pub name: String,
    pub image: String,
    pub running: bool,
    pub status: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct Row {
    #[serde(rename = "ID")]
    id: String,
    names: String,
    image: String,
    state: String,
    status: String,
}

pub fn installed() -> bool {
    std::env::var_os("PATH").map_or(false, |p| std::env::split_paths(&p).any(|d| d.join("docker").is_file()))
        || std::path::Path::new("/usr/bin/docker").is_file()
}

/// One JSON object per line, as printed by `docker ps -a --format '{{json .}}'`.
pub fn parse(output: &str) -> Vec<Container> {
    let mut v: Vec<Container> = output
        .lines()
        .filter_map(|l| serde_json::from_str::<Row>(l).ok())
        .map(|r| Container { id: r.id, name: r.names, image: r.image, running: r.state == "running", status: r.status })
        .collect();
    v.sort_by(|a, b| b.running.cmp(&a.running).then(a.name.cmp(&b.name)));
    v
}

pub enum Listing {
    Ok(Vec<Container>),
    /// The command failed: usually the daemon is not running or the user is not in the `docker` group.
    Error(String),
}

pub fn list() -> Listing {
    match Command::new("docker").args(["ps", "-a", "--format", "{{json .}}"]).output() {
        Ok(o) if o.status.success() => Listing::Ok(parse(&String::from_utf8_lossy(&o.stdout))),
        Ok(o) => Listing::Error(String::from_utf8_lossy(&o.stderr).lines().next().unwrap_or("docker failed").to_string()),
        Err(e) => Listing::Error(e.to_string()),
    }
}

pub fn action(verb: &str, id: &str) -> bool {
    matches!(verb, "start" | "stop" | "restart") && Command::new("docker").args([verb, id]).status().map_or(false, |s| s.success())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_ps_output_running_first() {
        let out = r#"{"ID":"a1","Names":"zeta","Image":"redis","State":"exited","Status":"Exited (0) 2 hours ago"}
{"ID":"b2","Names":"web","Image":"nginx:1","State":"running","Status":"Up 3 minutes"}
not json"#;
        let v = parse(out);
        assert_eq!(v.len(), 2);
        assert_eq!(v[0].name, "web");
        assert!(v[0].running && !v[1].running);
    }

    #[test]
    fn rejects_unknown_verbs() {
        assert!(!action("rm", "x"));
    }
}
