//! Calendar events from `.ics` files and links (Google Calendar's "secret address in iCal format",
//! Nextcloud, Outlook... all publish one). Reads events and expands repeating ones.
use chrono::{Datelike, Duration, Local, NaiveDate, NaiveDateTime, TimeZone, Weekday};

#[derive(Clone, Debug, PartialEq)]
pub struct Event {
    pub title: String,
    pub location: String,
    pub start: NaiveDateTime,
    pub end: NaiveDateTime,
    pub all_day: bool,
}

#[derive(Clone, Debug)]
struct Rule {
    freq: String,
    interval: i64,
    count: Option<usize>,
    until: Option<NaiveDateTime>,
    by_day: Vec<Weekday>,
}

#[derive(Clone, Debug)]
pub struct RawEvent {
    title: String,
    location: String,
    start: NaiveDateTime,
    end: NaiveDateTime,
    all_day: bool,
    rule: Option<Rule>,
    exdates: Vec<NaiveDateTime>,
}

fn unfold(text: &str) -> Vec<String> {
    let mut out: Vec<String> = vec![];
    for line in text.replace("\r\n", "\n").lines() {
        if (line.starts_with(' ') || line.starts_with('\t')) && !out.is_empty() {
            out.last_mut().unwrap().push_str(&line[1..]);
        } else {
            out.push(line.to_string());
        }
    }
    out
}

fn unescape(s: &str) -> String {
    s.replace("\\n", " ").replace("\\N", " ").replace("\\,", ",").replace("\\;", ";").replace("\\\\", "\\")
}

/// "20261107T100000Z", "20261107T100000" or "20261107" (all-day). Times in UTC are converted to local time.
fn parse_dt(value: &str) -> Option<(NaiveDateTime, bool)> {
    let v = value.trim();
    if v.len() == 8 {
        return NaiveDate::parse_from_str(v, "%Y%m%d").ok().map(|d| (d.and_hms_opt(0, 0, 0).unwrap(), true));
    }
    let utc = v.ends_with('Z');
    let naive = NaiveDateTime::parse_from_str(v.trim_end_matches('Z'), "%Y%m%dT%H%M%S").ok()?;
    if utc {
        let local = chrono::Utc.from_utc_datetime(&naive).with_timezone(&Local).naive_local();
        return Some((local, false));
    }
    Some((naive, false))
}

fn weekday(code: &str) -> Option<Weekday> {
    match &code[code.len().saturating_sub(2)..] {
        "MO" => Some(Weekday::Mon),
        "TU" => Some(Weekday::Tue),
        "WE" => Some(Weekday::Wed),
        "TH" => Some(Weekday::Thu),
        "FR" => Some(Weekday::Fri),
        "SA" => Some(Weekday::Sat),
        "SU" => Some(Weekday::Sun),
        _ => None,
    }
}

fn parse_rule(v: &str) -> Option<Rule> {
    let mut r = Rule { freq: String::new(), interval: 1, count: None, until: None, by_day: vec![] };
    for part in v.split(';') {
        let (k, val) = part.split_once('=')?;
        match k {
            "FREQ" => r.freq = val.to_string(),
            "INTERVAL" => r.interval = val.parse().unwrap_or(1).max(1),
            "COUNT" => r.count = val.parse().ok(),
            "UNTIL" => r.until = parse_dt(val).map(|x| x.0),
            "BYDAY" => r.by_day = val.split(',').filter_map(weekday).collect(),
            _ => {}
        }
    }
    if matches!(r.freq.as_str(), "DAILY" | "WEEKLY" | "MONTHLY" | "YEARLY") { Some(r) } else { None }
}

pub fn parse_ics(text: &str) -> Vec<RawEvent> {
    let mut out = vec![];
    let mut cur: Option<(String, String, Option<(NaiveDateTime, bool)>, Option<(NaiveDateTime, bool)>, Option<Rule>, Vec<NaiveDateTime>)> = None;
    for line in unfold(text) {
        if line == "BEGIN:VEVENT" {
            cur = Some((String::new(), String::new(), None, None, None, vec![]));
            continue;
        }
        if line == "END:VEVENT" {
            if let Some((title, loc, Some((start, all_day)), end, rule, exdates)) = cur.take() {
                let end = end.map(|e| e.0).unwrap_or(if all_day { start + Duration::days(1) } else { start });
                out.push(RawEvent { title, location: loc, start, end, all_day, rule, exdates });
            }
            cur = None;
            continue;
        }
        let Some(c) = cur.as_mut() else { continue };
        let Some((name, value)) = line.split_once(':') else { continue };
        let key = name.split(';').next().unwrap_or(name);
        match key {
            "SUMMARY" => c.0 = unescape(value),
            "LOCATION" => c.1 = unescape(value),
            "DTSTART" => c.2 = parse_dt(value),
            "DTEND" => c.3 = parse_dt(value),
            "RRULE" => c.4 = parse_rule(value),
            "EXDATE" => c.5.extend(value.split(',').filter_map(|v| parse_dt(v).map(|x| x.0))),
            _ => {}
        }
    }
    out
}

fn months_between(a: NaiveDate, b: NaiveDate) -> i64 {
    (b.year() as i64 - a.year() as i64) * 12 + b.month() as i64 - a.month() as i64
}

fn matches_day(r: &Rule, start: NaiveDate, day: NaiveDate) -> bool {
    let days = (day - start).num_days();
    if days < 0 {
        return false;
    }
    match r.freq.as_str() {
        "DAILY" => days % r.interval == 0,
        "WEEKLY" => {
            let wk = |d: NaiveDate| (d - Duration::days(d.weekday().num_days_from_monday() as i64)).signed_duration_since(NaiveDate::from_ymd_opt(1970, 1, 5).unwrap()).num_days() / 7;
            let in_week = if r.by_day.is_empty() { day.weekday() == start.weekday() } else { r.by_day.contains(&day.weekday()) };
            in_week && (wk(day) - wk(start)) % r.interval == 0
        }
        "MONTHLY" => day.day() == start.day() && months_between(start, day) % r.interval == 0,
        "YEARLY" => day.month() == start.month() && day.day() == start.day() && (day.year() - start.year()) as i64 % r.interval == 0,
        _ => false,
    }
}

/// Events (including every repetition) that overlap `[from, to)`, sorted by start.
pub fn expand(raw: &[RawEvent], from: NaiveDateTime, to: NaiveDateTime) -> Vec<Event> {
    let mut out = vec![];
    for e in raw {
        let len = e.end - e.start;
        let mk = |start: NaiveDateTime| Event { title: e.title.clone(), location: e.location.clone(), start, end: start + len, all_day: e.all_day };
        match &e.rule {
            None => {
                if e.start < to && e.end.max(e.start + Duration::seconds(1)) > from {
                    out.push(mk(e.start));
                }
            }
            Some(r) => {
                let mut day = e.start.date();
                let last = to.date();
                let mut n = 0usize;
                while day <= last && n < 20_000 {
                    if matches_day(r, e.start.date(), day) {
                        let start = day.and_time(e.start.time());
                        n += 1;
                        if r.count.map_or(false, |c| n > c) || r.until.map_or(false, |u| start > u) {
                            break;
                        }
                        if !e.exdates.contains(&start) && start < to && start + len > from {
                            out.push(mk(start));
                        }
                    }
                    day += Duration::days(1);
                }
            }
        }
    }
    out.sort_by_key(|e| e.start);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dt(y: i32, m: u32, d: u32, h: u32, mi: u32) -> NaiveDateTime {
        NaiveDate::from_ymd_opt(y, m, d).unwrap().and_hms_opt(h, mi, 0).unwrap()
    }

    const ICS: &str = "BEGIN:VCALENDAR\r\nBEGIN:VEVENT\r\nSUMMARY:Reunión\\, equipo\r\nDTSTART:20261110T100000\r\nDTEND:20261110T110000\r\nLOCATION:Sala 1\r\nEND:VEVENT\r\n\
BEGIN:VEVENT\r\nSUMMARY:Gimnasio\r\nDTSTART:20261102T180000\r\nDTEND:20261102T190000\r\nRRULE:FREQ=WEEKLY;BYDAY=MO,WE;COUNT=6\r\nEND:VEVENT\r\n\
BEGIN:VEVENT\r\nSUMMARY:Cumple\r\nDTSTART;VALUE=DATE:20261115\r\nEND:VEVENT\r\nEND:VCALENDAR";

    #[test]
    fn reads_simple_and_all_day_events() {
        let raw = parse_ics(ICS);
        assert_eq!(raw.len(), 3);
        let ev = expand(&raw, dt(2026, 11, 10, 0, 0), dt(2026, 11, 11, 0, 0));
        assert_eq!(ev.len(), 1);
        assert_eq!(ev[0].title, "Reunión, equipo");
        assert_eq!(ev[0].location, "Sala 1");
        let birthday = expand(&raw, dt(2026, 11, 15, 0, 0), dt(2026, 11, 16, 0, 0));
        assert!(birthday.iter().any(|e| e.title == "Cumple" && e.all_day));
    }

    #[test]
    fn weekly_rule_with_days_and_count() {
        let raw = parse_ics(ICS);
        let ev = expand(&raw, dt(2026, 11, 1, 0, 0), dt(2026, 12, 31, 0, 0));
        let gym: Vec<_> = ev.iter().filter(|e| e.title == "Gimnasio").collect();
        assert_eq!(gym.len(), 6); // COUNT=6: Mon 2, Wed 4, Mon 9, Wed 11, Mon 16, Wed 18
        assert_eq!(gym[0].start, dt(2026, 11, 2, 18, 0));
        assert_eq!(gym[1].start, dt(2026, 11, 4, 18, 0));
        assert_eq!(gym[5].start, dt(2026, 11, 18, 18, 0));
    }

    #[test]
    fn daily_interval_until_and_exdate() {
        let ics = "BEGIN:VEVENT\nSUMMARY:X\nDTSTART:20261101T080000\nDTEND:20261101T083000\nRRULE:FREQ=DAILY;INTERVAL=2;UNTIL=20261109T235959\nEXDATE:20261105T080000\nEND:VEVENT";
        let ev = expand(&parse_ics(ics), dt(2026, 11, 1, 0, 0), dt(2026, 12, 1, 0, 0));
        let days: Vec<u32> = ev.iter().map(|e| e.start.day()).collect();
        assert_eq!(days, vec![1, 3, 7, 9]);
    }

    #[test]
    fn monthly_and_yearly() {
        let m = "BEGIN:VEVENT\nSUMMARY:M\nDTSTART:20260115T090000\nRRULE:FREQ=MONTHLY\nEND:VEVENT";
        let ev = expand(&parse_ics(m), dt(2026, 11, 1, 0, 0), dt(2027, 1, 1, 0, 0));
        assert_eq!(ev.iter().map(|e| (e.start.month(), e.start.day())).collect::<Vec<_>>(), vec![(11, 15), (12, 15)]);
        let y = "BEGIN:VEVENT\nSUMMARY:Y\nDTSTART:20200704T000000\nRRULE:FREQ=YEARLY\nEND:VEVENT";
        let ev = expand(&parse_ics(y), dt(2026, 7, 1, 0, 0), dt(2026, 8, 1, 0, 0));
        assert_eq!(ev.len(), 1);
    }

    #[test]
    fn events_outside_the_range_are_left_out_and_lines_unfold() {
        let ics = "BEGIN:VEVENT\nSUMMARY:Largo\n título\nDTSTART:20300101T100000\nEND:VEVENT";
        let raw = parse_ics(ics);
        assert_eq!(raw[0].title, "Largotítulo"); // a line that starts with a space continues the previous one
        assert!(expand(&raw, dt(2026, 1, 1, 0, 0), dt(2026, 12, 31, 0, 0)).is_empty());
    }
}
