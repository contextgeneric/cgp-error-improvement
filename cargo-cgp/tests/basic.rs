use cargo_cgp::test_utils::test_cgp_error_from_json;
use insta::assert_snapshot;

#[test]
fn test_base_area_error() {
    let outputs = test_cgp_error_from_json("base_area.json", "base_area");

    // We expect one error message for base_area
    assert_eq!(outputs.len(), 1, "Expected 1 error message");

    assert_snapshot!(outputs[0], @"
     x missing field `heig�t` in the context `Rectangle`.
       ,-[examples/src/base_area.rs:41:9]
    40 |     CanUseRectangle for Rectangle {
    41 |         AreaCalculatorComponent,
       :         ^^^^^^^^^^^|^^^^^^^^^^^
       :                    `-- unsatisfied trait bound
    42 |     }
       `----
     help: Context `Rectangle` is missing a required field to use `AreaCalculatorComponent`.
               note: Missing field: `heig�t`
           
           note: some characters in the field name are hidden by the compiler and shown as '�'
           
           The struct `Rectangle` is defined at `examples/src/base_area.rs:41` but does not have the required field `heig�t`.
           
           Dependency chain:
               `CanUseRectangle` for `Rectangle` (check trait)
               └─ consumer trait of `AreaCalculatorComponent` for `Rectangle` (consumer trait)
                  └─ `AreaCalculator<Rectangle>` for provider `RectangleArea` (provider trait)
                     └─ `HasRectangleFields` for `Rectangle` (getter trait)
                        └─ field `heig�t` on `Rectangle` ✗
           
           To fix this error:
               • Add a field `heig�t` to the `Rectangle` struct at examples/src/base_area.rs:41
    ");
}

#[test]
fn test_base_area_2_error() {
    let outputs = test_cgp_error_from_json("base_area_2.json", "base_area_2");

    // We expect one error message for base_area_2
    assert_eq!(outputs.len(), 1, "Expected 1 error message");

    // This test case has no other HasField implementations,
    // so the error message should suggest adding the derive
    assert!(
        outputs[0].contains("is either missing the field")
            || outputs[0].contains("needs `#[derive(HasField)]`"),
        "Expected error message to mention missing derive possibility"
    );

    assert_snapshot!(outputs[0], @"
     x missing field `width` or `#[derive(HasField)]` in the context `Rectangle`.
       ,-[examples/src/base_area_2.rs:41:9]
    40 |     CanUseRectangle for Rectangle {
    41 |         AreaCalculatorComponent,
       :         ^^^^^^^^^^^|^^^^^^^^^^^
       :                    `-- unsatisfied trait bound
    42 |     }
       `----
     help: Context `Rectangle` is missing a required field to use `AreaCalculatorComponent`.
               note: Missing field: `width` or struct needs `#[derive(HasField)]`
           
           The struct `Rectangle` is defined at `examples/src/base_area_2.rs:41` but does not have the required field `width`.
           
           Dependency chain:
               `CanUseRectangle` for `Rectangle` (check trait)
               └─ consumer trait of `AreaCalculatorComponent` for `Rectangle` (consumer trait)
                  └─ `AreaCalculator<Rectangle>` for provider `RectangleArea` (provider trait)
                     └─ `HasRectangleFields` for `Rectangle` (getter trait)
                        └─ field `width` on `Rectangle` ✗
           
           To fix this error:
               • If the struct has the field `width`, add `#[derive(HasField)]` to the struct definition at `examples/src/base_area_2.rs:41`
               • If the field is missing, add a `width` field to the struct
    ");
}

#[test]
fn test_scaled_area_error() {
    let outputs = test_cgp_error_from_json("scaled_area.json", "scaled_area");

    // Now correctly merged into one error message
    assert_eq!(outputs.len(), 1, "Expected 1 error message");

    // The error should be the comprehensive CGP-formatted missing field error
    assert!(
        outputs[0].contains("missing field `height`"),
        "Error should be about missing height field"
    );

    assert_snapshot!(outputs[0], @"
     x missing field `height` in the context `Rectangle`.
       ,-[examples/src/scaled_area.rs:58:9]
    57 |     CanUseRectangle for Rectangle {
    58 |         AreaCalculatorComponent,
       :         ^^^^^^^^^^^|^^^^^^^^^^^
       :                    `-- unsatisfied trait bound
    59 |     }
       `----
     help: Context `Rectangle` is missing a required field to use `AreaCalculatorComponent`.
               note: Missing field: `height`
           
           The struct `Rectangle` is defined at `examples/src/scaled_area.rs:58` but does not have the required field `height`.
           
           Dependency chain:
               `CanUseRectangle` for `Rectangle` (check trait)
               └─ consumer trait of `AreaCalculatorComponent` for `Rectangle` (consumer trait)
                  └─ `AreaCalculator<Rectangle>` for provider `ScaledArea<RectangleArea>` (provider trait)
                     ├─ `HasRectangleFields` for `Rectangle` (getter trait)
                     │  └─ field `height` on `Rectangle` ✗
                     └─ `AreaCalculator<Rectangle>` for inner provider `RectangleArea` (provider trait) ✓
           
           The error in the higher-order provider `ScaledArea<RectangleArea>` might be caused by its inner provider `RectangleArea`.
           
           To fix this error:
               • Add a field `height` to the `Rectangle` struct at examples/src/scaled_area.rs:58
    ");
}

#[test]
fn test_scaled_area_2_error() {
    let outputs = test_cgp_error_from_json("scaled_area_2.json", "scaled_area_2");

    assert_eq!(outputs.len(), 1, "Expected 1 error message");

    assert_snapshot!(outputs[0], @"
     x missing field `scale_factor` in the context `Rectangle`.
       ,-[examples/src/scaled_area_2.rs:58:9]
    57 |     CanUseRectangle for Rectangle {
    58 |         AreaCalculatorComponent,
       :         ^^^^^^^^^^^|^^^^^^^^^^^
       :                    `-- unsatisfied trait bound
    59 |     }
       `----
     help: Context `Rectangle` is missing a required field to use `AreaCalculatorComponent`.
               note: Missing field: `scale_factor`
           
           The struct `Rectangle` is defined at `examples/src/scaled_area_2.rs:58` but does not have the required field `scale_factor`.
           
           Dependency chain:
               `CanUseRectangle` for `Rectangle` (check trait)
               └─ consumer trait of `AreaCalculatorComponent` for `Rectangle` (consumer trait)
                  └─ `AreaCalculator<Rectangle>` for provider `ScaledArea<RectangleArea>` (provider trait)
                     └─ `HasScaleFactor` for `Rectangle` (getter trait)
                        └─ field `scale_factor` on `Rectangle` ✗
           
           To fix this error:
               • Add a field `scale_factor` to the `Rectangle` struct at examples/src/scaled_area_2.rs:58
    ");
}
