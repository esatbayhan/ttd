use clap::CommandFactory;
use ttd::cli::Cli;

#[test]
fn cli_exposes_expected_subcommands() {
    let command = Cli::command();
    let names: Vec<String> = command
        .get_subcommands()
        .map(|subcommand| subcommand.get_name().to_owned())
        .collect();

    assert_eq!(names, vec!["add", "list", "done", "search"]);
}

#[test]
fn cli_allows_running_without_a_subcommand() {
    let result = Cli::command().try_get_matches_from(["ttd"]);

    assert!(result.is_ok());
}
