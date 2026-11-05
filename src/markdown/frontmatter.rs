#[derive(Debug, Clone, Default)]
pub struct FrontmatterData {
    pub properties: Vec<(String, String)>,
}

impl FrontmatterData {
    pub fn is_empty(&self) -> bool {
        self.properties.is_empty()
    }

    pub fn get(&self, key_insensitive: &str) -> Option<&str> {
        self.properties
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(key_insensitive))
            .map(|(_, v)| v.as_str())
    }
}

/// Extracts YAML frontmatter if the markdown starts with `---` on the first line.
/// Returns (FrontmatterData, remaining_markdown_body).
pub fn extract_frontmatter(input: &str) -> (FrontmatterData, &str) {
    let trimmed_start = input.strip_prefix('\u{feff}').unwrap_or(input);
    let Some(after_open) = trimmed_start
        .strip_prefix("---\r\n")
        .or_else(|| trimmed_start.strip_prefix("---\n"))
    else {
        return (FrontmatterData::default(), trimmed_start);
    };

    // Find closing `---` or `...` on its own line
    let mut end_offset = None;
    let mut body_start = None;
    let mut cursor = 0usize;

    for line in after_open.split_inclusive('\n') {
        let trimmed_line = line.trim_end_matches(&['\r', '\n'][..]).trim();
        if trimmed_line == "---" || trimmed_line == "..." {
            end_offset = Some(cursor);
            body_start = Some(cursor + line.len());
            break;
        }
        cursor += line.len();
    }

    let (Some(yaml_end), Some(rest_start)) = (end_offset, body_start) else {
        return (FrontmatterData::default(), trimmed_start);
    };

    let yaml_block = &after_open[..yaml_end];
    let rest = &after_open[rest_start..];
    let fm = parse_yaml_frontmatter(yaml_block);
    (fm, rest)
}

fn parse_yaml_frontmatter(yaml: &str) -> FrontmatterData {
    let mut properties: Vec<(String, String)> = Vec::new();
    let mut current_key: Option<String> = None;
    let mut current_list: Vec<String> = Vec::new();

    for raw_line in yaml.lines() {
        let trimmed = raw_line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        // Check if it's a list continuation item `- value`
        if let Some(item) = trimmed.strip_prefix("- ") {
            if current_key.is_some() {
                current_list.push(clean_yaml_scalar(item));
                continue;
            }
        }

        // Flush previous list if any
        if let Some(k) = current_key.take() {
            if !current_list.is_empty() {
                properties.push((k, current_list.join(", ")));
                current_list.clear();
            }
        }

        if let Some((k_raw, v_raw)) = trimmed.split_once(':') {
            let key = k_raw.trim().to_string();
            let val = v_raw.trim();
            if val.is_empty() {
                current_key = Some(key);
            } else if val.starts_with('[') && val.ends_with(']') {
                let inner = &val[1..val.len() - 1];
                let items: Vec<String> = inner
                    .split(',')
                    .map(clean_yaml_scalar)
                    .filter(|s| !s.is_empty())
                    .collect();
                properties.push((key, items.join(", ")));
            } else {
                properties.push((key, clean_yaml_scalar(val)));
            }
        }
    }

    if let Some(k) = current_key.take() {
        if !current_list.is_empty() {
            properties.push((k, current_list.join(", ")));
        }
    }

    FrontmatterData { properties }
}

fn clean_yaml_scalar(s: &str) -> String {
    let t = s.trim();
    if (t.starts_with('"') && t.ends_with('"') && t.len() >= 2)
        || (t.starts_with('\'') && t.ends_with('\'') && t.len() >= 2)
    {
        t[1..t.len() - 1].to_string()
    } else {
        t.to_string()
    }
}
