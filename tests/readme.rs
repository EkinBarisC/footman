//! The README's example config, read by the parser it claims to describe.
//!
//! A config example is the part of a README people paste rather than read, so
//! it is the part that has to be true. It is also the part that rots first: a
//! key name, an Action field or a warning that has since changed shape shows up
//! here as a skipped Binding rather than as an error, which is exactly the
//! failure a reader would blame on themselves.

/// The first fenced `toml` block in the README, which is the config example.
///
/// Line endings are normalised first. Git hands a Windows checkout whatever
/// `core.autocrlf` says it should — the CI runner's README has CRLF in it and
/// this machine's does not — and a fence looked for as LF is not found in a
/// file that spells its line breaks CRLF. That is a difference between two
/// checkouts of the same commit, so it is one to absorb rather than to report.
fn example() -> String {
    let readme = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/README.md"))
        .expect("the README is beside Cargo.toml")
        .replace("\r\n", "\n");

    let after = readme
        .split_once("```toml\n")
        .expect("the README shows a config")
        .1;
    after
        .split_once("```")
        .expect("and closes the fence")
        .0
        .to_string()
}

#[test]
fn the_readme_config_loads_without_complaint() {
    let example = example();

    let loaded = footman::Config::parse(&example).expect("the README's config parses");

    assert!(
        loaded.warnings.is_empty(),
        "the README shows a config Footman complains about: {:?}",
        loaded.warnings
    );
}

/// Every Binding shown, not merely most of them. An invalid Binding is skipped
/// with a warning rather than refused (DESIGN.md §7), so a broken example would
/// otherwise load quietly and be short.
#[test]
fn every_binding_the_readme_shows_survives_loading() {
    let example = example();
    let shown = example.matches("[[binding]]").count();

    let loaded = footman::Config::parse(&example).expect("the README's config parses");

    assert!(shown > 0, "the README shows no Bindings at all");
    assert_eq!(loaded.config.bindings.sorted().len(), shown);
}

/// The comment beside `tap` lists what may go there, and a reader who picks one
/// of them should not find themselves reading a warning instead.
#[test]
fn the_tap_actions_the_readme_offers_are_the_ones_that_exist() {
    for tap in ["none", "escape"] {
        let loaded = footman::Config::parse(&format!("[hyper]\ntap = \"{tap}\"\n"))
            .expect("a hyper section parses");

        assert!(
            loaded.warnings.is_empty(),
            "the README offers tap = {tap:?}, which Footman does not take: {:?}",
            loaded.warnings
        );
    }
}
