
# Prompt

## Overview

We are following the directions in `13-shorten-report.report.md` to build a cargo-cgp crate that will intercept JSON output from cargo check with `--message-format=json` enabled, and display improved error messages for CGP code.

### Merge dependency chains

The 2 error messages in `density_3.rs` can be merged.

These test cases demonstrate the error messages that arise from transitive dependencies. The provider `DensityFromMassField` implements `DensityCalculator` by depending on the context to implement the consumer trait `CanCalculateArea`. 

Compared to `density.rs` and `density_2.rs`, `density_3.rs` contains `check_components!` for both `AreaCalculatorComponent` and `DensityCalculatorComponent`.

When analyzing only the error for `DensityCalculatorComponent`, Rust does not show further details on why the provider trait `AreaCalculator` is not implemented. But when analyzing the error for `AreaCalculatorComponent`, we can see that the failure in `DensityCalculatorComponent` is directly related to failure in `CalculatorComponent`.

We should generalize the error processing, so that it can relate CGP errors across transitive dependencies involving multiple CGP components, and merge related errors together.

Your changes must not affect the output from the other examples, in particular: `base_area.rs`, `base_area_2.rs`, `scaled_area.rs`, `scaled_area_2.rs`, `density.rs`, `density_2.rs`.

If the source JSON error message do not contain sufficient information for you to reconstruct the suggested error message, then omit those details. You must not hard code anything about user-provided code in the code base.

Following is the suggestion for improved error message for `density_3.rs`:

```
E0277

  × missing field `height` in the context `Rectangle`.
    ╭─[examples/src/density_3.rs:66:9]
    ╭─[examples/src/density_3.rs:67:9]
 66 │         AreaCalculatorComponent,
    ·         ───────────┬───────────
    ·                    ╰── unsatisfied trait bound
 67 │         DensityCalculatorComponent,
    ·         ─────────────┬────────────
    ·                      ╰── unsatisfied trait bound
 68 │     }
    ╰────
  help: Dependency chain:
          CanUseRectangle for Rectangle (check trait)
          └─ requires: consumer trait of `DensityCalculatorComponent` for `Rectangle` (consumer trait)
             └─ requires: DensityCalculator<Rectangle> for provider DensityFromMassField (provider trait)
          └─────└─ requires: CanCalculateArea for Rectangle (consumer trait)
                  └─ requires: HasRectangleFields for Rectangle (getter trait)
                     └─ requires: field `height` on Rectangle ✗
        
        To fix this error:
            • Add a field `height` to the `Rectangle` struct at examples/src/density_3.rs:66
```

## Things to keep note

Following are things that you should keep note of throughout the project.

### Generalized Error Processing

The code you write must be generalized to be able to handle all kinds of CGP error messages. There must not be any hard code identification of module, fields, or constructs coming from the user-provided code. You should only pattern match based on constructs from the CGP libraries, and from the Rust standard libraries.

### Graceful Error Handling

All code in the code base must handle errors gracefully. You must not write code that panics or call methods that may panic like `.unwrap()` or `.expect()`. The only exception to this is for test assertions in test code.

If an error may happen in a function, make it return `Result` instead of panicking or ignoring the error.

### Inline documentation

Write detailed inline documentation about what the code you wrote are doing. Explain the rationale, why it should be kept, and when can it be removed.

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
