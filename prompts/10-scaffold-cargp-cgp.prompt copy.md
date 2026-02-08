We will follow the directions in the given reports to build a new cargo-cgp crate that will intercept JSON output from cargo check with `--message-format=json` enabled, and display improved error messages for CGP code.

First, we will scaffold the cargo-cgp project with a basic command line that forwards the plain `cargo cgp check` command to `cargo check`. The initial CLI do not need to worry about handling any additional CLI arguments.

When calling `cargo check`, cargo-cgp should pass on an additional `--message-format=json` argument to ensure that the output can be parsed as JSON.

Cargo-cgp should parse the JSON output from `cargo check`, and format it as human-readable output back to the user. 

Initially, we will not perform any processing on the output, including error messages. Instead, the main focus is to ensure that we can run `cargo cgp check` and get back the same output as `cargo check`.

You don't need to write any test for now. But ensure that the code compiles through every step of your development.