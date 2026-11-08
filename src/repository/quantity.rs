//! Kubernetes CPU and memory quantity parsing.

pub(crate) fn parse_quantity(value: &str) -> Option<f64> {
    let value = value.trim();
    if value.is_empty() {
        return None;
    }
    let split = value
        .find(|character: char| {
            !(character.is_ascii_digit()
                || character == '.'
                || character == '-'
                || character == '+')
        })
        .unwrap_or(value.len());
    let (number, suffix) = value.split_at(split);
    let number: f64 = number.parse().ok()?;
    let multiplier = match suffix {
        "" => 1.0,
        "n" => 1e-9,
        "u" => 1e-6,
        "m" => 1e-3,
        "k" => 1e3,
        "M" => 1e6,
        "G" => 1e9,
        "T" => 1e12,
        "P" => 1e15,
        "E" => 1e18,
        "Ki" => 1024.0,
        "Mi" => 1024.0 * 1024.0,
        "Gi" => 1024.0 * 1024.0 * 1024.0,
        "Ti" => 1024.0 * 1024.0 * 1024.0 * 1024.0,
        "Pi" => 1024.0 * 1024.0 * 1024.0 * 1024.0 * 1024.0,
        "Ei" => 1024.0 * 1024.0 * 1024.0 * 1024.0 * 1024.0 * 1024.0,
        _ => {
            let exponent = suffix
                .strip_prefix('e')
                .or_else(|| suffix.strip_prefix('E'))?;
            let exponent: i32 = exponent.parse().ok()?;
            10f64.powi(exponent)
        }
    };
    Some(number * multiplier)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_quantity(value: &str, expected: f64) {
        let parsed = parse_quantity(value).expect("quantity parses");
        assert!(
            (parsed - expected).abs() < f64::EPSILON * 8.0,
            "{value} parsed as {parsed}, expected {expected}"
        );
    }

    #[test]
    fn parses_cpu_and_memory_quantities() {
        assert_quantity("250m", 0.25);
        assert_quantity("150000000n", 0.15);
        assert_quantity("2", 2.0);
        assert_quantity("1Ki", 1024.0);
        assert_quantity("2Gi", 2.0 * 1024.0 * 1024.0 * 1024.0);
        assert_quantity("1e3", 1000.0);
        assert_eq!(parse_quantity(""), None);
    }
}
