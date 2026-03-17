use std::io::{self, Read};

fn sanitize_for_output(input: &str) -> String {
    input.escape_default().to_string()
}

fn main() -> io::Result<()> {
    let mut input = String::new();
    io::stdin().read_to_string(&mut input)?;

    let body = input.trim();
    let normalized = if body.is_empty() {
        "<empty>".to_string()
    } else {
        sanitize_for_output(body)
    };

    println!("Review status: ok");
    println!("Reviewer: example-review-plugin");
    println!("Summary: deterministic worker plugin example");
    println!("Input: {normalized}");

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::sanitize_for_output;

    #[test]
    fn escapes_control_characters() {
        assert_eq!(sanitize_for_output("hi\nthere"), "hi\\nthere");
        assert_eq!(sanitize_for_output("\u{1b}[31mred"), "\\u{1b}[31mred");
    }
}
