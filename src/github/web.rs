//! Reads the few profile details GitHub only exposes on the public web page:
//! achievements, highlights, the profile's time zone and the Stars tab count. There's no API for these, so this parses the
//! HTML and treats anything it can't find as absent rather than failing.

use super::models::{Achievement, ProfileExtras};
use anyhow::Result;

pub fn profile_extras(login: &str) -> Result<ProfileExtras> {
    let html = reqwest::blocking::Client::builder()
        .user_agent(concat!("Rivet/", env!("CARGO_PKG_VERSION")))
        .timeout(std::time::Duration::from_secs(30))
        .build()?
        .get(format!("https://github.com/{}", urlencoding::encode(login)))
        .send()?
        .error_for_status()?
        .text()?;
    Ok(parse_profile(&html))
}

pub fn parse_profile(html: &str) -> ProfileExtras {
    ProfileExtras {
        utc_offset_hours: attr_after(html, "data-hours-ahead-of-utc=\"", 0).and_then(|v| v.parse().ok()),
        achievements: achievements(html),
        highlights: highlights(html),
        stars: tab_count(html, "stars"),
    }
}

/// Value of an attribute starting at the first `marker` after `from`.
fn attr_after<'a>(html: &'a str, marker: &str, from: usize) -> Option<&'a str> {
    let start = html.get(from..)?.find(marker)? + from + marker.len();
    let end = html[start..].find('"')? + start;
    Some(&html[start..end])
}

fn strip_tags(s: &str) -> String {
    let mut out = String::new();
    let mut in_tag = false;
    for c in s.chars() {
        match c {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => out.push(c),
            _ => {}
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ").replace("&amp;", "&")
}

/// The tab counters carry the exact number in a `title` right before `class="Counter"`.
fn tab_count(html: &str, tab: &str) -> Option<u64> {
    let tab_at = html.find(&format!("tab={tab}\""))?;
    let rest = &html[tab_at..];
    let counter = rest.find("class=\"Counter\"")?;
    let title = rest[..counter].rfind("title=\"")? + "title=\"".len();
    let end = rest[title..].find('"')? + title;
    rest[title..end].replace(',', "").parse().ok()
}

fn achievements(html: &str) -> Vec<Achievement> {
    let Some(start) = html.find(">Achievements</a></h2>") else { return vec![] };
    let block = &html[start..];
    // the sidebar block ends at its closing div; the same badges repeat later for small screens
    let block = &block[..block.find("</div>").unwrap_or(block.len())];
    block
        .split("<a href=")
        .skip(1)
        .filter_map(|item| {
            let image = attr_after(item, "src=\"", 0)?.to_string();
            let name = attr_after(item, "alt=\"Achievement: ", 0)?.to_string();
            let tier = item.find("achievement-tier-label").and_then(|i| {
                let gt = item[i..].find('>')? + i + 1;
                let lt = item[gt..].find('<')? + gt;
                Some(item[gt..lt].trim().to_string()).filter(|t| !t.is_empty())
            });
            Some(Achievement { name, image, tier })
        })
        .collect()
}

fn highlights(html: &str) -> Vec<String> {
    let Some(start) = html.find(">Highlights</h2>") else { return vec![] };
    let block = &html[start..];
    let block = &block[..block.find("</ul>").unwrap_or(block.len())];
    // drop the rest of the opening <li ...> tag before stripping
    block.split("<li").skip(1).map(|li| strip_tags(li.split_once('>').map_or(li, |x| x.1))).filter(|t| !t.is_empty()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"
        <a href="/u?tab=projects"> Projects <span title="0" class="Counter">0</span></a>
        <a href="/u?tab=stars"> Stars <span title="1,329" data-view-component="true" class="Counter">1.3k</span></a>
        <profile-timezone class="x" data-hours-ahead-of-utc="3.0">
        <h2><a href="/u?tab=achievements">Achievements</a></h2><div class="d-flex flex-wrap">
          <a href="/u?achievement=pull-shark"><img src="https://a/pull-shark.png" width="64" alt="Achievement: Pull Shark" /><span class="Label achievement-tier-label">x3</span></a>
          <a href="/u?achievement=yolo"><img src="https://a/yolo.png" alt="Achievement: YOLO" /></a>
        </div>
        <h2 class="mb-2 h4">Highlights</h2><ul class="list-style-none"><li class="mt-2"> <a href="x"> <svg></svg> Developer Program Member </a> </li> </ul>
    "#;

    #[test]
    fn parses_profile_extras() {
        let p = parse_profile(SAMPLE);
        assert_eq!(p.utc_offset_hours, Some(3.0));
        assert_eq!(p.stars, Some(1329));
        assert_eq!(p.achievements.len(), 2);
        assert_eq!(p.achievements[0].name, "Pull Shark");
        assert_eq!(p.achievements[0].tier.as_deref(), Some("x3"));
        assert_eq!(p.achievements[1].tier, None);
        assert_eq!(p.highlights, vec!["Developer Program Member"]);
    }
}
