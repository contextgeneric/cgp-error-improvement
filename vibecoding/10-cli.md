# Prompt

## Overview

We are following the directions in `10-combined-report.report.md` to build a cargo-cgp crate that will intercept JSON output from cargo check with `--message-format=json` enabled, and display improved error messages for CGP code.

In the task, you will refactor the code base and make the following improvements.

## Handle `--message-format` CLI

Add and handle a `--message-format` CLI argument that follows the same behavior as `cargo check`. Following is the documentation for the CLI flag.

```
--message-format fmt
    The output format for diagnostic messages. Can be specified multiple times and consists of comma-separated values. Valid values:
        human (default): Display in a human-readable text format. Conflicts with short and json.
        short: Emit shorter, human-readable text messages. Conflicts with human and json.
        json: Emit JSON messages to stdout. See the reference for more details. Conflicts with human and short.
        json-diagnostic-short: Ensure the rendered field of JSON messages contains the “short” rendering from rustc. Cannot be used with human or short.
        json-diagnostic-rendered-ansi: Ensure the rendered field of JSON messages contains embedded ANSI color codes for respecting rustc’s default color scheme. Cannot be used with human or short.
        json-render-diagnostics: Instruct Cargo to not include rustc diagnostics in JSON messages printed, but instead Cargo itself should render the JSON diagnostics coming from rustc. Cargo’s own JSON diagnostics and others coming from rustc are still emitted. Cannot be used with human or short.
```

To support all options, you should first investigate how Cargo handles each option. If possible, reuse any existing library code provided by Cargo or cargo-metadata to implement this.

If supporting all options are too complicated, at minimum you must support the `json` format. If you leave out support for any option, provide detailed reasonings.

## Generalized Error Processing

The code you write must be generalized to be able to handle all kinds of CGP error messages. There must not be any hard code identification of module, fields, or constructs coming from the user-provided code. You should only pattern match based on constructs from the CGP libraries, and from the Rust standard libraries.

## Graceful Error Handling

All code in the code base must handle errors gracefully. You must not write code that panics or call methods that may panic like `.unwrap()` or `.expect()`. The only exception to this is for test assertions in test code.

## Testing

You should run existing unit tests and check if there is any change in output. After your changes are finalized, update the relevant `assert_snapshot` using the appropriate `cargo insta` command.

If the output has changed, ensure that the error messages for existing test cases do not get worse.

You should also test running `cargo-cgp` against the example code by running `target/debug/cargo-cgp cgp check`. Before running, edit `lib.rs` to uncomment the specific example module that you want to test, while comment out all other example modules.

Ensure that the output is really improved, such as all CGP error messages are formatted properly, and that there is no more dangling output from the previous line.

## Planning

Before you start, write a detailed plan on the changes you are going to make in your response.
