use std::io::{self, Read};

fn main() -> io::Result<()> {
    let mut input = String::new();
    io::stdin().read_to_string(&mut input)?;

    let body = input.trim();
    let normalized = if body.is_empty() { "<empty>" } else { body };

    println!("Review status: ok");
    println!("Reviewer: example-review-plugin");
    println!("Summary: deterministic worker plugin example");
    println!("Input: {normalized}");

    Ok(())
}
