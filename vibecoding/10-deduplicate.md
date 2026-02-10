# Prompt

## Overview

We are following the directions in `10-combined-report.report.md` to build a cargo-cgp crate that will intercept JSON output from cargo check with `--message-format=json` enabled, and display improved error messages for CGP code.

In the task, you will refactor the code base and make the following improvements.

## Merging Errors

The test in `test_scaled_area_error` is getting two CGP error messages instead of merging them into one. Fix the error merging logic in `DiagnosticDatabase` to handle this correctly.

## Improve higher-order provider error tracing

In the example `scaled_area.rs`, the error is directly related to the implementation of the `RectangleArea` provider, but the CGP error message only shows that the failure is on the composed `ScaledArea<RectangleArea>` provider.

When multiple providers are shown in the source error, you should trace and see whether one provider is the inner composition of the other provider. e.g. `RectangleArea` is the inner provider of `ScaledArea<RectangleArea>`. In such case, you should hint that the error in `ScaledArea<RectangleArea>` is likely caused by the error in `RectangleArea`.

## Incorrect Line Number

The line number reported in the CGP errors do not match the line in the original source code. Try to find the correct line number metadata from the source error.

## Help and Note Rendering

The current rendering of multi-line help is not very nice. Each line should be placed in its own `help` entry.

The `note` entries should be at the same level as the `help`. If that is not possible, convert the `note` to `help`.

## Things to keep note

Following are things that you should keep note of throughout the project.

### Generalized Error Processing

The code you write must be generalized to be able to handle all kinds of CGP error messages. There must not be any hard code identification of module, fields, or constructs coming from the user-provided code. You should only pattern match based on constructs from the CGP libraries, and from the Rust standard libraries.

### Graceful Error Handling

All code in the code base must handle errors gracefully. You must not write code that panics or call methods that may panic like `.unwrap()` or `.expect()`. The only exception to this is for test assertions in test code.

If an error may happen in a function, make it return `Result` instead of panicking or ignoring the error.

### Unit Test

You should run existing unit tests and check if there is any change in output. After your changes are finalized, update the relevant `assert_snapshot` using the appropriate `cargo insta` command.

The number of expected output in the tests should not change, unless specified in the instruction. If you get more error than originally expected, this means that the error merging logic has changed somewhere and should be fixed.

Ensure that all unit test uses `assert_snapshot!` to test the expected output. Add any missing `assert_snapshot!` to match the number of expected outputs.

If the output has changed, ensure that the error messages for existing test cases do not get worse.

### End-to-end Test

You should test running `cargo-cgp` against the example code by running `target/debug/cargo-cgp cgp check`. Before running, edit `lib.rs` to uncomment the specific example module that you want to test, while comment out all other example modules.

Ensure that the output is really improved, such as all CGP error messages are formatted properly, and that there is no more dangling output from the previous line.

## Planning

Before you start, write a detailed plan on the changes you are going to make in your response.
