use cargo_cgp::test_utils::test_cgp_error_from_json;
use insta::assert_snapshot;

#[test]
fn test_density_error() {
    let outputs = test_cgp_error_from_json("density.json", "density");

    assert_eq!(outputs.len(), 1, "Expected 1 error message");

    assert_snapshot!(outputs[0], @"
    E0277

      x the trait bound `RectangleArea: AreaCalculator<Rectangle>` is not satisfied
        ,-[examples/src/density.rs:64:9]
     63 |     
     64 | ,-> check_components! {
     65 | |->     CanUseRectangle for Rectangle {
        : `---- unsatisfied trait bound
     66 |             DensityCalculatorComponent,
        `----
      help: Dependency chain:
              CanUseRectangle for Rectangle (check trait)
              └─ requires: consumer trait of `DensityCalculatorComponent` for `Rectangle` (consumer trait)
                 └─ requires: DensityCalculator<Rectangle> for provider DensityFromMassField (provider trait)
                    └─ requires: CanCalculateArea for Rectangle (consumer trait)
                       └─ requires: AreaCalculator<Rectangle> for provider RectangleArea (provider trait) ✗
            
            Add a check that `Rectangle` can use `CalculateAreaComponent` using `check_components!` to get further details on the missing dependencies.
    ");
}

#[test]
fn test_density_2_error() {
    let outputs = test_cgp_error_from_json("density_2.json", "density_2");

    assert_eq!(outputs.len(), 1, "Expected 1 error message");

    assert_snapshot!(outputs[0], @"
    E0277

      x the trait bound `ScaledArea<RectangleArea>: AreaCalculator<Rectangle>` is not satisfied
        ,-[examples/src/density_2.rs:80:9]
     79 |     
     80 | ,-> check_components! {
     81 | |->     CanUseRectangle for Rectangle {
        : `---- unsatisfied trait bound
     82 |             DensityCalculatorComponent,
        `----
      help: Dependency chain:
              CanUseRectangle for Rectangle (check trait)
              └─ requires: consumer trait of `DensityCalculatorComponent` for `Rectangle` (consumer trait)
                 └─ requires: DensityCalculator<Rectangle> for provider DensityFromMassField (provider trait)
                    └─ requires: CanCalculateArea for Rectangle (consumer trait)
                       └─ requires: AreaCalculator<Rectangle> for provider ScaledArea<RectangleArea> (provider trait) ✗
            
            Add a check that `Rectangle` can use `CalculateAreaComponent` using `check_components!` to get further details on the missing dependencies.
    ");
}

#[test]
fn test_density_3_error() {
    let outputs = test_cgp_error_from_json("density_3.json", "density_3");

    assert_eq!(outputs.len(), 1, "Expected 1 error message (merged)");

    assert_snapshot!(outputs[0], @"
    E0277

      x missing field `height` in the context `Rectangle`.
        ,-[examples/src/density_3.rs:66:9]
     65 |     CanUseRectangle for Rectangle {
     66 |         AreaCalculatorComponent,
        :         ^^^^^^^^^^^|^^^^^^^^^^^
        :                    `-- unsatisfied trait bound
     67 |         DensityCalculatorComponent,
        :         ^^^^^^^^^^^^^|^^^^^^^^^^^^
        :                      `-- unsatisfied trait bound
     68 |     }
        `----
      help: Context `Rectangle` is missing a required field to use multiple components: `AreaCalculatorComponent`, `DensityCalculatorComponent`.
                note: Missing field: `height`
            
            The struct `Rectangle` is defined at `examples/src/density_3.rs:66` but does not have the required field `height`.
            
            Dependency chain:
                CanUseRectangle for Rectangle (check trait)
                ├─ requires: CanCalculateArea for Rectangle (consumer trait)
                │  └─ requires: AreaCalculator<Rectangle> for provider RectangleArea (provider trait)
                │     └─ requires: HasRectangleFields for Rectangle (getter trait)
                │        └─ requires: field `height` on Rectangle ✗
                └─ requires: consumer trait of `DensityCalculatorComponent` for `Rectangle` (consumer trait)
                   └─ requires: DensityCalculator<Rectangle> for provider DensityFromMassField (provider trait)
                      └─ requires: CanCalculateArea for Rectangle (consumer trait) (*)
            
            To fix this error:
                • Add a field `height` to the `Rectangle` struct at examples/src/density_3.rs:66
    ");
}
