
# Prompt

## Overview

We are following the directions in `13-shorten-report.report.md` to build a cargo-cgp crate that will intercept JSON output from cargo check with `--message-format=json` enabled, and display improved error messages for CGP code.

## Task

### Incorrect Consumer Trait

The error message is not displaying the correct consumer trait name. When the consumer trait cannot be found, it should instead be written as "consumer trait of {ComponentName}".

For example, in `base_area.rs`, the following should be displayed:

```
E0277

  × missing field `heig�t` in the context `Rectangle`.
    ╭─[examples/src/base_area.rs:41:9]
 40 │     CanUseRectangle for Rectangle {
 41 │         AreaCalculatorComponent,
    ·         ───────────┬───────────
    ·                    ╰── unsatisfied trait bound
 42 │     }
    ╰────
  help: Context `Rectangle` is missing a required field to use `AreaCalculatorComponent`.
            note: Missing field: `heig�t`
        
        note: some characters in the field name are hidden by the compiler and shown as '�'
        
        The struct `Rectangle` is defined at `examples/src/base_area.rs:41` but does not have the required field `heig�t`.
        
        Dependency chain:
            CanUseRectangle for Rectangle (check trait)
            └─ requires: consumer trait of `AreaCalculatorComponent` for Rectangle (consumer trait)
               └─ requires: AreaCalculator<Rectangle> for provider RectangleArea (provider trait)
                  └─ requires: HasRectangleFields for Rectangle (getter trait)
                     └─ requires: field `heig�t` on Rectangle ✗
        
        To fix this error:
            • Add a field `heig�t` to the `Rectangle` struct at examples/src/base_area.rs:41
```


### Missing intermediate dependency

The error messages for `density.rs` and `density_2.rs` do not contain sufficient details in the dependency chain.

These test cases demonstrate the error messages that arise from transitive dependencies. The provider `DensityFromMassField` implements `DensityCalculator` by depending on the context to implement the consumer trait `CanCalculateArea`. But when there is a missing dependency in the provider `RectangleArea`, Rust does not show further details on why the provider trait `AreaCalculator` is not implemented.

We will need to perform arbitrarily deep rendering of transitive dependencies, to uncover the problematic provider to the extend that we can. Additionally, we should add a help message that asks the user to check for the implementation of the indirect component like `AreaCalculatorComponent` in their check trait.

In this example, we can also see that the consumer trait name `CanCalculateArea` is visible from the error log, but the consumer trait name `CanCalculateDensity` is hidden.

Following is the suggestion for improved error message for `density.rs`:

```
E0277

  × the trait bound `RectangleArea: AreaCalculator<Rectangle>` is not satisfied
    ╭─[examples/src/density.rs:66:9]
 65 │     CanUseRectangle for Rectangle {
 66 │         DensityCalculatorComponent,
    ·         ─────────────┬────────────
    ·                      ╰── unsatisfied trait bound
 67 │     }
    ╰────
  help: Dependency chain:
      └─ requires: CanUseRectangle for Rectangle (check trait)
         └─ requires: consumer trait of `DensityCalculatorComponent` for `Rectangle` (consumer trait)
            └─ requires: `DensityCalculator<Rectangle>` for provider `DensityFromMassField` (provider trait)
               └─ requires: `CanCalculateArea` for `Rectangle` (consumer trait)
                  └─ requires: `AreaCalculator<Rectangle>` for provider `RectangleArea` (provider trait) ✗
   
  help: Add a check that `Rectangle` can use `AreaCalculatorComponent` using `check_components!` to get further details on the missing dependencies.
```

Following is the suggestion for improved error message for `density_2.rs`:

```
E0277

  × the trait bound `ScaledArea<RectangleArea>: AreaCalculator<Rectangle>` is not satisfied
    ╭─[examples/src/density_2.rs:82:9]
 81 │     CanUseRectangle for Rectangle {
 82 │         DensityCalculatorComponent,
    ·         ─────────────┬────────────
    ·                      ╰── unsatisfied trait bound
 83 │     }
    ╰────
  help: Dependency chain:
      └─ requires: CanUseRectangle for Rectangle (check trait)
         └─ requires: consumer trait of `DensityCalculatorComponent` for `Rectangle` (consumer trait)
            └─ requires: `DensityCalculator<Rectangle>` for provider `DensityFromMassField` (provider trait)
               └─ requires: `CanCalculateArea` for `Rectangle` (consumer trait)
                  └─ requires: `AreaCalculator<Rectangle>` for provider `ScaledArea<RectangleArea>` (provider trait) ✗

  help: Add a check that `Rectangle` can use `AreaCalculatorComponent` using `check_components!` to get further details on the missing dependencies.
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
