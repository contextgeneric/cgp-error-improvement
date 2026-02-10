# Prompt

## Overview

We are following the directions in `10-combined-report.report.md` to build a cargo-cgp crate that will intercept JSON output from cargo check with `--message-format=json` enabled, and display improved error messages for CGP code.

In the task, you will refactor the code base and make the following improvements.

## Field Name Display

In examples like `base_area.rs`, some of the characters in `HasField` are omitted, such as `HasField<Symbol<6, cgp::prelude::Chars<'h', cgp::prelude::Chars<'e', cgp::prelude::Chars<'i', cgp::prelude::Chars<'g', cgp::prelude::Chars<_, cgp::prelude::Chars<'t', Nil>>>>>>>>`.

Currently, this is parsed as `heigt`, with the unknown charater skipped. But we should instead replace `_` with `�` and show it like `heig�t`.

When there are unknown characters in `HasField`, also add an additional note that some characters are hidden from the source error and is shown as `�`. But do not show this message if there is no unknown characters.

Aside from this, the type-level character may contain any valid `char` value. So the field name parser should handle all possible variations. When the characters contain non-basic identifier values, e.g. `[^a-zA-Z0-9\-\_�]`, consider rendering the field name the same way Rust strings are rendered in the terminal.

## Collecting and rendering `CompilerMessage`

Currently, the `render_compiler_message` function accepts one `CompilerMessage` and returns a `Result<String, Error>`. This is then called through `render_message` inside a for loop in `run_check`. This flow needs to be improved to better handle the error messages.

For each of these `CompilerMessage`, if the error is related to CGP, then process and store it in `DiagnosticDatabase`, but do not render anything. Otherwise if the error is not related to CGP, then render it immediately.

When processing a `CompilerMessage`, `DiagnosticDatabase` should determine if the message is related to an existing entry in the database. If so, it would use the given `CompilerMessage` to add additional information to the existing entry, and it should not create new entry in that case.

After the source stream has ended, then only we display all CGP-related error messages in one go at the end `DiagnosticDatabase` would provide a `render_cgp_errors` method that formats and return all CGP error messages as a `Vec<String>`. This would then be printed to the terminal by the caller.

There is an additional issue which is that there should be only one instance of `DiagnosticDatabase` throughout the entire lifetime of the program. So the database should be created outside of the main for loop, and passed around to the functions for rendering or processing.

## Testing

You should run existing unit tests and check if there is any change in output. After your changes are finalized, update the relevant `assert_snapshot` using the appropriate `cargo insta` command.

If the output has changed, ensure that the error messages for existing test cases do not get worse.

## Planning

Before you start, write a detailed plan on the changes you are going to make in your response.