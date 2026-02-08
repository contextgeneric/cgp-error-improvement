We will follow the directions in the given reports to build a new cargo-cgp crate that will intercept JSON output from cargo check with `--message-format=json` enabled, and display improved error messages for CGP code.

Currently, we have scaffolded a `render_compiler_message` function that formats a `CompilerMessage` into a human-readable string. The implementation currently simply return the `rendered` field from `Diagnostic`.

You are tasked to rewrite `render_compiler_message` to produce better error messages for CGP code.

When the error message does not involve the use of CGP, the code should simply return the original error message without any modifications.

For the initial work, focus only on improving the error message for `base_area.rs`, with the JSON output stored in `base_area.json`, and the original human-readable error message stored in `base_area.log`.

To test the implementation, you should write a unit test that reads directly from `base_area.json`, parse it and calls `render_compiler_message`, and then print the output using `println!`. You should then run the test using `cargo test -p cargo-cgp -- --nocapture` to check the output of the improved error message.

You should not test or verify your code by compiling and running the `cargo-cgp` binary. Instead, this work can be verified solely based on the unit test that processes the `base_area.json` fixture.