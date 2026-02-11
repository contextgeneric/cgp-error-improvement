use cargo_cgp::diagnostic_db::DiagnosticDatabase;
use cargo_cgp::error_formatting::render_diagnostic_plain;
use cargo_cgp::render::render_message;
use cargo_metadata::Message;
use insta::assert_snapshot;
use std::fs::File;
use std::io::BufReader;

/// Helper function to run a CGP error test from a JSON file
fn test_cgp_error_from_json(json_filename: &str, test_name: &str) -> Vec<String> {
    // Read the JSON fixture (newline-delimited JSON)
    let json_path = format!(
        "{}/../examples/src/{}",
        env!("CARGO_MANIFEST_DIR"),
        json_filename
    );

    println!("\n=== Testing {} ===", test_name);
    println!("Reading JSON from: {}", json_path);
    let file =
        File::open(&json_path).unwrap_or_else(|_| panic!("Failed to open {}", json_filename));

    let reader = BufReader::new(file);

    let mut output_lines = Vec::new();

    let mut db = DiagnosticDatabase::new();

    for message in Message::parse_stream(reader) {
        let message = message.expect("Failed to parse message");
        render_message(&message, &mut db);
    }

    let cgp_diagnostics = db.render_cgp_diagnostics();
    for diagnostic in cgp_diagnostics {
        let rendered = render_diagnostic_plain(&diagnostic);
        println!("{}", rendered);
        output_lines.push(rendered);
    }

    // Return the output for snapshot testing
    output_lines
}

#[test]
fn test_base_area_error() {
    let outputs = test_cgp_error_from_json("base_area.json", "base_area");

    // We expect one error message for base_area
    assert_eq!(outputs.len(), 1, "Expected 1 error message");

    assert_snapshot!(outputs[0], @"
    E0277

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
                CanUseRectangle for Rectangle (check trait)
                └─ requires: AreaCalculator<Rectangle> for Rectangle (consumer trait)
                   └─ requires: AreaCalculator<Rectangle> for provider RectangleArea (provider trait)
                      └─ requires: HasRectangleFields for Rectangle (getter trait)
                         └─ requires: field `heig�t` on Rectangle ✗
            
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
    E0277

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
                CanUseRectangle for Rectangle (check trait)
                └─ requires: AreaCalculator<Rectangle> for Rectangle (consumer trait)
                   └─ requires: AreaCalculator<Rectangle> for provider RectangleArea (provider trait)
                      └─ requires: HasRectangleFields for Rectangle (getter trait)
                         └─ requires: field `width` on Rectangle ✗
            
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
    E0277

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
                CanUseRectangle for Rectangle (check trait)
                └─ requires: AreaCalculator<Rectangle> for Rectangle (consumer trait)
                   └─ requires: AreaCalculator<Rectangle> for provider ScaledArea<RectangleArea> (provider trait)
                      ├─ requires: HasRectangleFields for Rectangle (getter trait)
                      │  └─ requires: field `height` on Rectangle ✗
                      └─ requires: AreaCalculator<Rectangle> for inner provider RectangleArea (provider trait) ✓
            
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
    E0277

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
                CanUseRectangle for Rectangle (check trait)
                └─ requires: AreaCalculator<Rectangle> for Rectangle (consumer trait)
                   └─ requires: AreaCalculator<Rectangle> for provider ScaledArea<RectangleArea> (provider trait)
                      └─ requires: HasScaleFactor for Rectangle (getter trait)
                         └─ requires: field `scale_factor` on Rectangle ✗
            
            To fix this error:
                • Add a field `scale_factor` to the `Rectangle` struct at examples/src/scaled_area_2.rs:58
    ");
}
