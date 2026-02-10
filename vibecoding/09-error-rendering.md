# Prompt

## Overview

We are following the directions in `10-combined-report.report.md` to build a cargo-cgp crate that will intercept JSON output from cargo check with `--message-format=json` enabled, and display improved error messages for CGP code.

In the task, you will refactor the code base and make the following improvements.

## Display errors with `miette`

Follow the suggestions in `10-combined-report.report.md` to use `miette` to display the errors in more colorful ways. 

To do this, you may need to modify `format_error_message` to return a miette-specific type instead of `String`. 

When displaying error messages, check that if `cargo-cgp` is running from the terminal, then render the error message in a colourful way using `miette`. Otherwise, render the error message as plain text as before.

When rendering error messages in test and JSON, always use plain text mode without the terminal color modifiers.

Both colourful and plain text errors must be rendered through the same data structure through `miette`. The only difference between the two is one has terminal escape characters to provide color, and the other only use miette to display plain text.

The errors displayed through `miette` should be the same CGP error messages that we have worked hard to render. Do not use `miette` to render the original error messages.

You should remove all existing ad hoc error rendering code, and replace them to use miette. You must ensure that the error messages rendered for both color and plain text mode go through the same code path, and produce the same textual content.

You should also ensure that the miette color mode is enabled when running cargo-cgp from the terminal.

You are free to update the formats and expected output from existing tests to match the new output that is produced through `miette`. But make sure that the new expected in the tests are still showing CGP errors instead of the original errors.

When doing the refactoring, also consider how we can make use of `miette` to render the error messages in different structures that is better suited for rendering with `miette`.

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
