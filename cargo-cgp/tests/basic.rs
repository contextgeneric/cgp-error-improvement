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
       ,-[examples/src/base_area.rs:1:9]
     1 |         AreaCalculatorComponent,
       :         ^^^^^^^^^^^|^^^^^^^^^^^
       :                    `-- unsatisfied trait bound
       `----
      help: note: some characters in the field name are hidden by the compiler and shown as '�'
            the struct `Rectangle` is missing the required field `heig�t`
            ensure a field `heig�t` of the appropriate type is present in the `Rectangle` struct
            note: this field is required by the trait bound `CanUseRectangle`
            note: delegation chain:
              - required for `Rectangle` to implement `HasRectangleFields`
              - required for `RectangleArea` to implement the provider trait `AreaCalculator`
              - required for `Rectangle` to implement `the consumer trait `CanUseRectangle`
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
       ,-[examples/src/base_area_2.rs:1:9]
     1 |         AreaCalculatorComponent,
       :         ^^^^^^^^^^^|^^^^^^^^^^^
       :                    `-- unsatisfied trait bound
       `----
      help: the struct `Rectangle` is either missing the field `width` or is missing `#[derive(HasField)]`
            ensure a field `width` of the appropriate type is present in the `Rectangle` struct, or add `#[derive(HasField)]` if the struct is missing the derive
            note: this field is required by the trait bound `CanUseRectangle`
            note: delegation chain:
              - required for `Rectangle` to implement `HasRectangleFields`
              - required for `RectangleArea` to implement the provider trait `AreaCalculator`
              - required for `Rectangle` to implement `the consumer trait `CanUseRectangle`
    ");
}

#[test]
fn test_scaled_area_error() {
    let outputs = test_cgp_error_from_json("scaled_area.json", "scaled_area");

    // FIXME: should merge the two error messages into one.

    assert_eq!(outputs.len(), 2, "Expected 2 error messages");

    // The first error is about the provider trait relationship
    assert!(
        outputs[0].contains("delegation chain") || outputs[0].contains("AreaCalculator"),
        "First error should contain delegation or AreaCalculator information"
    );

    // The second error should be the comprehensive CGP-formatted missing field error
    assert!(
        outputs[1].contains("missing field `height`"),
        "Second error should be about missing height field"
    );

    // The delegation chain should be deduplicated -
    // should not redundantly mention both ScaledArea<RectangleArea> and RectangleArea
    let delegation_chain_part = outputs[1]
        .split("delegation chain:")
        .nth(1)
        .expect("Expected delegation chain section");

    // Count how many times "AreaCalculator" appears in provider trait mentions
    let area_calculator_count = delegation_chain_part
        .matches("provider trait `AreaCalculator`")
        .count();

    // Should only mention the provider trait once (not for both ScaledArea and RectangleArea)
    assert!(
        area_calculator_count <= 1,
        "Delegation chain should not redundantly mention the same provider trait multiple times. Found {} mentions.",
        area_calculator_count
    );

    assert_snapshot!(outputs[0], @"
    E0277

      x the trait bound `RectangleArea: AreaCalculator<Rectangle>` is not satisfied
       ,-[examples/src/scaled_area.rs:1:9]
     1 |         AreaCalculatorComponent,
       :         ^^^^^^^^^^^|^^^^^^^^^^^
       :                    `-- unsatisfied trait bound
       `----
      help: note: delegation chain:
              - required for `ScaledArea<RectangleArea>` to implement the provider trait `AreaCalculator`
              - required for `Rectangle` to implement `the consumer trait `CanUseRectangle`
    ");

    assert_snapshot!(outputs[1], @"
    missing field `height` required by CGP component
        Diagnostic severity: error
    Begin snippet for examples/src/scaled_area.rs starting at line 1, column 1

    snippet line 1:         AreaCalculatorComponent,
        label at line 1, columns 9 to 31: unsatisfied trait bound
    diagnostic help: the struct `Rectangle` is missing the required field `height`
    ensure a field `height` of the appropriate type is present in the `Rectangle` struct
    note: this field is required by the trait bound `CanUseRectangle`
    note: delegation chain:
      - required for `Rectangle` to implement `HasRectangleFields`
      - required for `ScaledArea<RectangleArea>` to implement the provider trait `AreaCalculator`
      - required for `Rectangle` to implement `the consumer trait `CanUseRectangle`
    diagnostic code: E0277
    ");
}

#[test]
fn test_scaled_area_2_error() {
    let outputs = test_cgp_error_from_json("scaled_area_2.json", "scaled_area_2");

    assert_eq!(outputs.len(), 1, "Expected 1 error message");

    assert_snapshot!(outputs[0], @"
    E0277

      x missing field `scale_factor` required by CGP component
       ,-[examples/src/scaled_area_2.rs:1:9]
     1 |         AreaCalculatorComponent,
       :         ^^^^^^^^^^^|^^^^^^^^^^^
       :                    `-- unsatisfied trait bound
       `----
      help: the struct `Rectangle` is missing the required field `scale_factor`
            ensure a field `scale_factor` of the appropriate type is present in the `Rectangle` struct
            note: this field is required by the trait bound `CanUseRectangle`
            note: delegation chain:
              - required for `Rectangle` to implement `HasScaleFactor`
              - required for `ScaledArea<RectangleArea>` to implement the provider trait `AreaCalculator`
              - required for `Rectangle` to implement `the consumer trait `CanUseRectangle`
    ");
}
