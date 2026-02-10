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

      x missing field `heig�t` (possibly incomplete) required by CGP component
        ,-[examples/src/base_area.rs:41:9]
     40 |     CanUseRectangle for Rectangle {
     41 |         AreaCalculatorComponent,
        :         ^^^^^^^^^^^|^^^^^^^^^^^
        :                    `-- unsatisfied trait bound
     42 |     }
        `----
      help: note: some characters in the field name are hidden by the compiler and shown as '�'
            the struct `Rectangle` is missing the required field `heig�t`
            ensure a field `heig�t` of the appropriate type is present in the `Rectangle` struct
            note: this field is required by the trait bound `CanUseRectangle`
            note: delegation chain:
              required for `Rectangle` to implement `HasRectangleFields`
              required for `RectangleArea` to implement the provider trait `AreaCalculator`
              required for `Rectangle` to implement `the consumer trait `CanUseRectangle`
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

      x missing field `width` required by CGP component
        ,-[examples/src/base_area_2.rs:41:9]
     40 |     CanUseRectangle for Rectangle {
     41 |         AreaCalculatorComponent,
        :         ^^^^^^^^^^^|^^^^^^^^^^^
        :                    `-- unsatisfied trait bound
     42 |     }
        `----
      help: the struct `Rectangle` is either missing the field `width` or is missing `#[derive(HasField)]`
            ensure a field `width` of the appropriate type is present in the `Rectangle` struct, or add `#[derive(HasField)]` if the struct is missing the derive
            note: this field is required by the trait bound `CanUseRectangle`
            note: delegation chain:
              required for `Rectangle` to implement `HasRectangleFields`
              required for `RectangleArea` to implement the provider trait `AreaCalculator`
              required for `Rectangle` to implement `the consumer trait `CanUseRectangle`
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

      x missing field `height` required by CGP component
        ,-[examples/src/scaled_area.rs:58:9]
     57 |     CanUseRectangle for Rectangle {
     58 |         AreaCalculatorComponent,
        :         ^^^^^^^^^^^|^^^^^^^^^^^
        :                    `-- unsatisfied trait bound
     59 |     }
        `----
      help: the struct `Rectangle` is missing the required field `height`
            ensure a field `height` of the appropriate type is present in the `Rectangle` struct
            note: this field is required by the trait bound `CanUseRectangle`
            note: delegation chain:
              the error in `ScaledArea<RectangleArea>` is likely caused by the inner provider `RectangleArea`
              required for `ScaledArea<RectangleArea>` to implement the provider trait `AreaCalculator`
              required for `Rectangle` to implement `the consumer trait `CanUseRectangle`
              required for `Rectangle` to implement `HasRectangleFields`
    ");
}

#[test]
fn test_scaled_area_2_error() {
    let outputs = test_cgp_error_from_json("scaled_area_2.json", "scaled_area_2");

    assert_eq!(outputs.len(), 1, "Expected 1 error message");

    assert_snapshot!(outputs[0], @"
    E0277

      x missing field `scale_factor` required by CGP component
        ,-[examples/src/scaled_area_2.rs:58:9]
     57 |     CanUseRectangle for Rectangle {
     58 |         AreaCalculatorComponent,
        :         ^^^^^^^^^^^|^^^^^^^^^^^
        :                    `-- unsatisfied trait bound
     59 |     }
        `----
      help: the struct `Rectangle` is missing the required field `scale_factor`
            ensure a field `scale_factor` of the appropriate type is present in the `Rectangle` struct
            note: this field is required by the trait bound `CanUseRectangle`
            note: delegation chain:
              required for `Rectangle` to implement `HasScaleFactor`
              required for `ScaledArea<RectangleArea>` to implement the provider trait `AreaCalculator`
              required for `Rectangle` to implement `the consumer trait `CanUseRectangle`
    ");
}
