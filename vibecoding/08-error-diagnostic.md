# Prompt

## Overview

We are following the directions in `10-combined-report.report.md` to build a cargo-cgp crate that will intercept JSON output from cargo check with `--message-format=json` enabled, and display improved error messages for CGP code.

In the task, you will refactor the code base and make the following improvements.

## Remove Deduplication Logic

`DiagnosticDatabase` currently contains a `deduplicate` method to suppress duplicate entries. You should remove it and move any deduplication logic to `add_diagnostic`, so that a duplicate `Diagnostic` is used only to update the existing entry.

## Format CGP errors back to `CompilerMessage`

In order for `cargo-cgp` itself to support the `--message-format=json` CLI argument, we need to update `DiagnosticDatabase` to convert its entries back to a list of `CompilerMessage`s.

Update the `DiagnosticDatabase` to process a `CompilerMessage` instead of the inner `Diagnostic`. The database should retain sufficient information so that it can reconstruct the `CompilerMessage`s.

Implement a `render_compiler_messages` method on `DiagnosticDatabase` to return a `Vec<CompilerMessage>`. The `DiagnosticDatabase` should use its own heuristics to write helpful metadata in `CompilerMessage` that aligns with the transformed human-readable CGP error messages.

Modify the `render_cgp_errors` to call `render_compiler_messages` to get back the `Vec<CompilerMessage>`, and then render the human-readable CGP errors based on that alone.

## Generalized Error Processing

The code you write must be generalized to be able to handle all kinds of CGP error messages. There must not be any hard code identification of module, fields, or constructs coming from the user-provided code. You should only pattern match based on constructs from the CGP libraries, and from the Rust standard libraries.

## Testing

You should run existing unit tests and check if there is any change in output. After your changes are finalized, update the relevant `assert_snapshot` using the appropriate `cargo insta` command.

If the output has changed, ensure that the error messages for existing test cases do not get worse.

## Planning

Before you start, write a detailed plan on the changes you are going to make in your response.