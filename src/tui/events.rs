pub fn normalize_key(key: &str) -> &str {
    match key {
        "return" => "enter",
        "escape" => "esc",
        "bs" | "del" => "backspace",
        raw => raw,
    }
}
