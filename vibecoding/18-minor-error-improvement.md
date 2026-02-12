# Prompt

## Overview

We are following the directions in `13-shorten-report.report.md` to build a cargo-cgp crate that will intercept JSON output from cargo check with `--message-format=json` enabled, and display improved error messages for CGP code.

## Task

### Error Formatting Improvement

You will implement the following improvements to the rendered CGP error messages:

Remove the error code like `E0277`.

Remove the `requires:` prefix in the dependency chain.

Always use backtick `` ` `` to quote code constructs in the help messages. 

For example:

```
Dependency chain:
      `CanUseRectangle` for `Rectangle` (check trait)
      └─ Consumer trait of `AreaCalculatorComponent` for `Rectangle` (consumer trait)
         └─ `AreaCalculator<Rectangle>` for provider `RectangleArea` (provider trait)
            └─ `HasRectangleFields` for `Rectangle` (getter trait)
               └─ Field `width` on `Rectangle` ✗
```

### Requirements

If the source JSON error message do not contain sufficient information for you to reconstruct the suggested error message, then omit those details. You must not hard code anything about user-provided code in the code base.

## Things to keep note

Following are things that you should keep note of throughout the project.

### Generalized Error Processing

The code you write must be generalized to be able to handle all kinds of CGP error messages. There must not be any hard code identification of module, fields, or constructs coming from the user-provided code. You should only pattern match based on constructs from the CGP libraries, and from the Rust standard libraries.

### Graceful Error Handling

All code in the code base must handle errors gracefully. You must not write code that panics or call methods that may panic like `.unwrap()` or `.expect()`. The only exception to this is for test assertions in test code.

If an error may happen in a function, make it return `Result` instead of panicking or ignoring the error.

### Inline documentation

Write detailed inline documentation about what the code you wrote are doing. Explain the rationale, why it should be kept, and when can it be removed.

Your inline comments should not mention user-provided constructs coming from the example code, such as `CanCalculateArea`.

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
