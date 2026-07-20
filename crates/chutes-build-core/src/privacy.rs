//! Conservative secret filtering for persistent Markdown memory.

const SECRET_MARKERS: &[&str] = &[
    "api_key",
    "apikey",
    "authorization:",
    "bearer ",
    "private_key",
    "client_secret",
    "access_token",
    "refresh_token",
    "password",
    "secret=",
    "cpk_",
    "sk-",
];

pub fn contains_probable_secret(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    SECRET_MARKERS.iter().any(|marker| lower.contains(marker))
}

pub fn filter_memory_markdown(text: &str) -> String {
    text.lines()
        .map(|line| {
            if contains_probable_secret(line) {
                "[redacted: probable secret]"
            } else {
                line
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redacts_probable_api_key_line() {
        let filtered = filter_memory_markdown("keep this\nCHUTES_API_KEY=cpk_secret\nalso keep");
        assert_eq!(
            filtered,
            "keep this\n[redacted: probable secret]\nalso keep"
        );
    }
}
