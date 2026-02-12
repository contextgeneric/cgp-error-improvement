
# Prompt

## Overview

We are following the directions in `13-shorten-report.report.md` to build a cargo-cgp crate that will intercept JSON output from cargo check with `--message-format=json` enabled, and display improved error messages for CGP code.

## Merge dependency chains

### Background

These test cases demonstrate the error messages that arise from transitive dependencies. The provider `DensityFromMassField` implements `DensityCalculator` by depending on the context to implement the consumer trait `CanCalculateArea`. 

Compared to `density.rs` and `density_2.rs`, `density_3.rs` contains `check_components!` for both `AreaCalculatorComponent` and `DensityCalculatorComponent`.

When analyzing only the error for `DensityCalculator`, Rust does not show further details on why the provider trait `AreaCalculator` is not implemented. But when analyzing the error for `AreaCalculator`, we can see that the failure in `DensityCalculator` is directly related to failure in `AreaCalculator`.

Essentially, this forms a directed acyclic graph, where `AreaCalculator` has both `DensityCalculator` and and the check trait `CanUseRectangle` as its parent. In practice, this dependency graph can grow arbitrarily deep.

### Task

To support more complicated dependency graph, we will flatten the dependency rendering, and render them in similar ways as `cargo tree`. This way, consumer traits from different source error message are rendered at the same level.

For the case of `density_3.rs`, it means that we will move `CanCalculateArea` and its children back to the same indentation level as `CanUseRectangle`. On the other hand, in the leaf node of `CanUseRectangle`, where we encounter `CanCalculateArea`, we will still render the entry but with an additional `(*)` to indicate that the children continues back at the top level.

When rendering this, the database should still keep track of the relationship between the component name and the consumer trait name. For example, when rendering the dependencies for `AreaCalculatorComponent`, we know that `Rectangle: CanCalculateArea` depends on `RectangleArea: AreaCalculator<Rectangle>`, thus the pattern indicates that `CanCalculateArea` is the consumer trait of `AreaCalculator`. This information would be stored for later lookup, so that we can recover the consumer trait name from the component name.

Additionally, the dependency graph should be rendered in the same order as the incoming error messages. So for example in `density_3.rs`, the error for `AreaCalculatorComponent` is shown first. 

Following is what the updated output for `density_3.rs` should look like:

```
E0277

  × missing field `height` in the context `Rectangle`.
    ╭─[examples/src/density_3.rs:66:9]
 65 │     CanUseRectangle for Rectangle {
 66 │         AreaCalculatorComponent,
    ·         ───────────┬───────────
    ·                    ╰── unsatisfied trait bound
 67 │         DensityCalculatorComponent,
    ·         ─────────────┬────────────
    ·                      ╰── unsatisfied trait bound
 68 │     }
    ╰────
  help: Context `Rectangle` is missing a required field to use multiple components: `AreaCalculatorComponent`, `DensityCalculatorComponent`.
            note: Missing field: `height`
        
        The struct `Rectangle` is defined at `examples/src/density_3.rs:66` but does not have the required field `height`.
        
        Dependency chain:
            CanUseRectangle for Rectangle (check trait)
            └─ requires: CanCalculateArea for Rectangle (consumer trait)
               └─ requires: AreaCalculator<Rectangle> for provider RectangleArea (provider trait)
                  └─ requires: HasRectangleFields for Rectangle (getter trait)
                     └─ requires: field `height` on Rectangle ✗
            └─ requires: consumer trait of `DensityCalculatorComponent` for `Rectangle` (consumer trait)
               └─ requires: DensityCalculator<Rectangle> for provider DensityFromMassField (provider trait)
                  └─ requires: CanCalculateArea for Rectangle (consumer trait) (*)

        To fix this error:
            • Add a field `height` to the `Rectangle` struct at examples/src/density_3.rs:66
```

### Requirements

Your changes must not affect the output from the other examples, in particular: `base_area.rs`, `base_area_2.rs`, `scaled_area.rs`, `scaled_area_2.rs`, `density.rs`, `density_2.rs`.

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
