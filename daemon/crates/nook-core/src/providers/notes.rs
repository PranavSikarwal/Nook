#![allow(dead_code)] // Warning: the next plan task uses these building blocks.
fn check_title(value: &str) -> Result<&str, ()> {
    let title = value
        .strip_suffix("\r\n")
        .or_else(|| value.strip_suffix('\n'))
        .unwrap_or(value);
    if title.is_empty() || title.chars().count() > 200 || title.contains(['\r', '\n', '\0']) {
        return Err(());
    }
    Ok(title)
}

fn check_body(value: &str) -> Result<&str, ()> {
    check_plain_text(value, 0, 20_000)
}

fn check_text(value: &str) -> Result<&str, ()> {
    check_plain_text(value, 1, 20_000)
}

fn check_plain_text(value: &str, min: usize, max: usize) -> Result<&str, ()> {
    let count = value.chars().count();
    if value.contains('\0') || count < min || count > max {
        return Err(());
    }
    Ok(value)
}

fn check_note_id(value: &str) -> Result<&str, ()> {
    if !(14..=300).contains(&value.chars().count())
        || !value.starts_with("x-coredata://")
        || !value
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || ":/._%-".contains(c))
    {
        return Err(());
    }
    Ok(value)
}

fn create_html(title: &str, body: &str) -> Result<String, ()> {
    Ok(format!(
        "<div>{}</div>{}",
        escape_html(title),
        html_lines(body)
    ))
}

fn append_html(text: &str) -> Result<String, ()> {
    Ok(html_lines(check_text(text)?))
}

fn html_lines(value: &str) -> String {
    value
        .split('\n')
        .map(|line| format!("<div>{}</div>", escape_html(line)))
        .collect()
}

fn escape_html(value: &str) -> String {
    value.chars().fold(String::new(), |mut escaped, character| {
        escaped.push_str(match character {
            '&' => "&amp;",
            '<' => "&lt;",
            '>' => "&gt;",
            _ => {
                escaped.push(character);
                return escaped;
            }
        });
        escaped
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn title_removes_one_trailing_line_break() {
        assert_eq!(check_title("Title\n").unwrap(), "Title");
        assert_eq!(check_title("Title\r\n").unwrap(), "Title");
    }

    #[test]
    fn title_refuses_empty_or_multiline_values() {
        for value in ["\n", "Title\n\n", "Ti\ntle"] {
            assert!(check_title(value).is_err());
        }
    }

    #[test]
    fn plain_text_checks_lengths_and_nul() {
        assert!(check_body("").is_ok());
        assert!(check_text("text").is_ok());
        assert!(check_body(&"x".repeat(20_001)).is_err());
        assert!(check_text("x\0y").is_err());
    }

    #[test]
    fn note_id_requires_prefix_and_allowed_characters() {
        assert!(check_note_id("x-coredata://ABC-123/._%:").is_ok());
        assert!(check_note_id("-e").is_err());
        assert!(check_note_id("ABC-x-coredata://123").is_err());
    }

    #[test]
    fn html_escapes_content_and_puts_title_first() {
        assert_eq!(
            create_html("T&<>", "a&<>").unwrap(),
            "<div>T&amp;&lt;&gt;</div><div>a&amp;&lt;&gt;</div>"
        );
        assert_eq!(append_html("a\nb").unwrap(), "<div>a</div><div>b</div>");
    }
}
