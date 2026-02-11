
# Prompt

## Overview

We are following the directions in `13-shorten-report.report.md` to build a cargo-cgp crate that will intercept JSON output from cargo check with `--message-format=json` enabled, and display improved error messages for CGP code.

## Task

Improve the error messages in `basic.rs` to follow more closely to the example ideal error messages shown below. In particular, show the dependencies in the form of a tree structure.

Make sure that you use the relevant features in `mitte` to do the formatting like `help:` and `note:`. Do not do ad hoc textual rendering other than for the dependency tree structure.

If the source JSON error message do not contain sufficient information for you to reconstruct the suggested error message, then omit those details. You must not hard code anything about user-provided code in the code base.

### `base_area.rs`

```
error[E0277]: missing field `heig�t` in the context `Rectangle`.

  --> examples/src/base_area.rs:41:9
   |
40 |     CanUseRectangle for Rectangle {
41 |         AreaCalculatorComponent,
   |         ^^^^^^^^^^^^^^^^^^^^^^^ component requires missing field
   |

help: Context `Rectangle` is missing a required field to use `AreaCalculatorComponent`.

help: Required field: `heig�t`
    note: some characters in the field name are hidden by the compiler and shown as '�'

help: The struct `Rectangle` is defined at `examples/src/base_area.rs:26` but does not have the required field `heig�t`.

help: Dependency chain:
    CanUseRectangle for Rectangle (check trait)
    └─ requires: CanCalculateArea for Rectangle (consumer trait)
        └─ requires: AreaCalculator<Rectangle> for provider RectangleArea (provider trait)
            └─ requires: HasRectangleFields for Rectangle (getter trait)
            └─ requires: field `heig�t` on Rectangle ✗

help: Available fields on `Rectangle`:
  • widt... ✓

help: To fix this error:
    • Add a field `height` to the `Rectangle` struct at examples/src/base_area.rs:26

```

### `base_area_2.rs`

```
error[E0277]: missing field `width` or `#[derive(HasField)]` in the context `Rectangle`.

  --> examples/src/base_area_2.rs:41:9
   |
40 |     CanUseRectangle for Rectangle {
41 |         AreaCalculatorComponent,
   |         ^^^^^^^^^^^^^^^^^^^^^^^ component requires field access
   |

help: Context `Rectangle` is missing field access implementations to use `AreaCalculatorComponent`.
    note: Missing field: `width`

help: Dependency chain:
    CanUseRectangle for Rectangle (check trait)
    └─ requires: CanCalculateArea for Rectangle (consumer trait)
        └─ requires: AreaCalculator<Rectangle> for provider RectangleArea (provider trait)
            └─ requires: HasRectangleFields for Rectangle (getter trait)
            └─ requires: field `width` on Rectangle ✗

help: Possible causes:
    1. The struct `Rectangle` is missing `#[derive(HasField)]`
    2. The field `width` does not exist on the struct

help: To fix this error:
    • If the struct has the field `width`, add `#[derive(HasField)]` to the struct definition at `examples/src/base_area_2.rs:27`
    • If the field is missing, add a `width` field to the struct
```

### `scaled_area.rs`

```
error[E0277]: missing field `height` in the context `Rectangle`.

  --> examples/src/scaled_area.rs:58:9
   |
57 |     CanUseRectangle for Rectangle {
58 |         AreaCalculatorComponent,
   |         ^^^^^^^^^^^^^^^^^^^^^^^ component requires missing field
   |

help: Context `Rectangle` is missing a required field to use `AreaCalculatorComponent`.
    note: Missing field: `height`

help: The struct `Rectangle` is defined at `examples/src/scaled_area.rs:42` but does not have the required field `height`.

Dependency chain:
  CanUseRectangle for Rectangle (check trait)
  └─ requires: CanCalculateArea for Rectangle (consumer trait)
     └─ requires: AreaCalculator<Rectangle> for provider ScaledArea<RectangleArea> (provider trait)
        └─ requires: AreaCalculator<Rectangle> for inner provider RectangleArea (provider trait)
           └─ requires: HasRectangleFields for Rectangle (getter trait)
              └─ requires: field `height` on Rectangle ✗

help: The error in the higher-order provider `ScaledArea<RectangleArea>` might be caused by its inner provider `RectangleArea`.

help: Available fields on `Rectangle`:
    • sca... ✓
    • wid... ✓
  
help: To fix this error:
    • Add a field `height` to the `Rectangle` struct at `examples/src/scaled_area.rs:42`
```

### `scaled_area_2.rs`

```
error[E0277]: missing field `scale_factor` required by CGP component

  --> examples/src/scaled_area_2.rs:58:9
   |
57 |     CanUseRectangle for Rectangle {
58 |         AreaCalculatorComponent,
   |         ^^^^^^^^^^^^^^^^^^^^^^^ component requires missing field
   |

help: Context `Rectangle` is missing a required field to use `AreaCalculatorComponent`.
    note: Missing field: `scale_factor`

help: The struct `Rectangle` is defined at `examples/src/scaled_area_2.rs:42`, but does not have the required field `scale_factor`.

Dependency chain:
  `CanUseRectangle` for `Rectangle` (check trait)
  └─ requires: consumer trait of `AreaCalculatorComponent` for `Rectangle` (consumer trait)
     └─ requires: `AreaCalculator<Rectangle>` for provider `ScaledArea<RectangleArea>` (provider trait)
        ├─ requires: HasScaleFactor for `Rectangle` (getter trait)
        │  └─ requires: field `scale_factor` on `Rectangle` ✗
        └─ requires: `AreaCalculator<Rectangle>` for inner provider `RectangleArea` ✓

Available fields on `Rectangle`:
  • wid... ✓
  • hei... ✓

To fix this error:
  • Add a field `scale_factor` to the `Rectangle` struct at `examples/src/scaled_area_2.rs:42`
```


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
