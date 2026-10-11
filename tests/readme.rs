const README: &str = include_str!("../README.md");
const EXAMPLES: [&str; 2] = [
    include_str!("../examples/quickstart.rs"),
    include_str!("../examples/gallery.rs"),
];

fn normalized(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[test]
fn readme_code_is_copied_from_the_examples() {
    let examples = normalized(&EXAMPLES.join("\n"));
    let blocks: Vec<&str> = README
        .split("```rust\n")
        .skip(1)
        .filter_map(|rest| rest.split("```").next())
        .collect();
    assert_eq!(blocks.len(), 8);
    for block in blocks {
        assert!(
            examples.contains(&normalized(block)),
            "this README block is not in examples/:\n{block}"
        );
    }
}
