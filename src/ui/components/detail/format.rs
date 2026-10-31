//! Pure formatting helpers shared by detail views and widgets.

/// Human-readable age for a Unix timestamp in seconds.
pub(crate) fn age(created_at: Option<i64>) -> String {
    let Some(created) = created_at else {
        return "—".to_string();
    };
    let secs = (jiff::Timestamp::now().as_second() - created).max(0) as u64;
    if secs >= 86_400 {
        format!("{}d", secs / 86_400)
    } else if secs >= 3_600 {
        format!("{}h", secs / 3_600)
    } else {
        format!("{}m", secs / 60)
    }
}

pub(crate) fn format_cpu(millicores: u64) -> String {
    if millicores >= 1000 {
        let cores = millicores as f64 / 1000.0;
        if (cores - cores.round()).abs() < f64::EPSILON {
            format!("{} cores", cores.round() as u64)
        } else {
            format!("{cores:.1} cores")
        }
    } else {
        format!("{millicores}m")
    }
}

pub(crate) fn format_bytes(bytes: u64) -> String {
    const KIB: f64 = 1024.0;
    const MIB: f64 = KIB * 1024.0;
    const GIB: f64 = MIB * 1024.0;
    const TIB: f64 = GIB * 1024.0;
    let bytes = bytes as f64;
    if bytes >= TIB {
        format!("{:.1} TiB", bytes / TIB)
    } else if bytes >= GIB {
        format!("{:.1} GiB", bytes / GIB)
    } else if bytes >= MIB {
        format!("{:.1} MiB", bytes / MIB)
    } else if bytes >= KIB {
        format!("{:.1} KiB", bytes / KIB)
    } else {
        format!("{} B", bytes.round() as u64)
    }
}

/// Relative label for a usage sample, e.g. `now` or `-30s`.
pub(crate) fn format_sample_label(observed_at: i64, latest: Option<i64>) -> String {
    let Some(latest) = latest else {
        return String::new();
    };
    let behind = (latest - observed_at).max(0);
    if behind == 0 {
        "now".to_string()
    } else {
        format!("-{behind}s")
    }
}

/// Turn a camelCase JSON key into title case for detail sections.
pub(crate) fn humanize(key: &str) -> String {
    let mut output = String::with_capacity(key.len() + 4);
    let mut previous_lowercase = false;
    for character in key.chars() {
        if character.is_ascii_uppercase() && previous_lowercase {
            output.push(' ');
        }
        previous_lowercase = character.is_ascii_lowercase();
        if output.is_empty() {
            output.extend(character.to_uppercase());
        } else {
            output.push(character);
        }
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn humanizes_camel_case_keys() {
        assert_eq!(humanize("binaryData"), "Binary Data");
        assert_eq!(humanize("spec"), "Spec");
        assert_eq!(humanize("podCIDRs"), "Pod CIDRs");
    }

    #[test]
    fn formats_cpu_and_bytes() {
        assert_eq!(format_cpu(500), "500m");
        assert_eq!(format_cpu(1000), "1 cores");
        assert_eq!(format_bytes(1024), "1.0 KiB");
    }
}
