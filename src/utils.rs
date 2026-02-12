pub fn is_compliant(current: &str, required: &str) -> bool {
    let clean_curr = current
        .trim_start_matches('v')
        .split('-')
        .next()
        .unwrap_or("");
    let clean_req = required
        .trim_start_matches('v')
        .split('-')
        .next()
        .unwrap_or("");

    let curr_parts: Vec<u32> = clean_curr
        .split('.')
        .filter_map(|s| s.parse().ok())
        .collect();
    let req_parts: Vec<u32> = clean_req
        .split('.')
        .filter_map(|s| s.parse().ok())
        .collect();

    if curr_parts.is_empty() || req_parts.is_empty() {
        return false;
    }

    // --- Lineage Protection ---
    // If requirement starts with 0. (Firedancer) but current starts with >= 1 (Agave)
    // Or vice versa. They are different software and cannot be compared by version number.
    if curr_parts[0] != req_parts[0] {
        return false; 
    }

    // Standard version comparison
    curr_parts >= req_parts
}

pub fn format_duration(seconds: i64) -> String {
    let days = seconds / 86400;
    let hours = (seconds % 86400) / 3600;
    let mins = (seconds % 3600) / 60;
    let secs = seconds % 60;
    if days > 0 {
        format!("{}d {}h {}m {}s", days, hours, mins, secs)
    } else if hours > 0 {
        format!("{}h {}m {}s", hours, mins, secs)
    } else {
        format!("{}m {}s", mins, secs)
    }
}
