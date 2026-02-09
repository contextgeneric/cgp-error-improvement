use cargo_cgp::render_compiler_message::render_compiler_message;
use cargo_metadata::Message;
use cargo_metadata::diagnostic::DiagnosticLevel;
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

    let mut error_count = 0;
    let mut compiler_message_count = 0;
    let mut total_messages = 0;
    let mut output_lines = Vec::new();

    // Parse the stream of JSON messages
    for message_result in Message::parse_stream(reader) {
        let message = message_result.expect("Failed to parse message");
        total_messages += 1;

        match &message {
            Message::CompilerMessage(compiler_msg) => {
                compiler_message_count += 1;

                // Process error-level diagnostics
                if matches!(compiler_msg.message.level, DiagnosticLevel::Error) {
                    error_count += 1;

                    println!("\n=== Original Error #{} ===", error_count);
                    if let Some(rendered) = &compiler_msg.message.rendered {
                        println!("{}", rendered);
                    }

                    println!("\n=== Improved CGP Error #{} ===", error_count);
                    match render_compiler_message(&compiler_msg) {
                        Ok(improved) => {
                            println!("{}", improved);
                            output_lines.push(improved);
                        }
                        Err(e) => {
                            println!("Error rendering: {}", e);
                            panic!("Failed to render compiler message: {}", e);
                        }
                    }
                }
            }
            _ => {}
        }
    }

    println!("\n=== Summary for {} ===", test_name);
    println!("Total messages parsed: {}", total_messages);
    println!("Compiler messages found: {}", compiler_message_count);
    println!("Error messages found: {}", error_count);

    assert!(
        compiler_message_count > 0,
        "Expected to find at least one compiler message in {}",
        json_filename
    );

    // Return the output for snapshot testing
    output_lines
}

#[test]
fn test_base_area_error() {
    let outputs = test_cgp_error_from_json("base_area.json", "base_area");

    // We expect one error message for base_area
    assert_eq!(outputs.len(), 1, "Expected 1 error message");

    assert_snapshot!(outputs[0], @"
    error[E0277]: missing field `heigt` (possibly incomplete) required by CGP component
      --> examples/src/base_area.rs:41:9
       |
      41 |         AreaCalculatorComponent,
         |         ^^^^^^^^^^^^^^^^^^^^^^^ unsatisfied trait bound
       |
       = help: struct `Rectangle` is missing the field `heigt`
       = note: this field is required by the trait bound `CanUseRectangle`
       = note: delegation chain:
               - required for `Rectangle` to implement `HasRectangleFields`
               - required for `RectangleArea` to implement the provider trait `AreaCalculator`
               - required for `Rectangle` to implement `the consumer trait `CanUseRectangle`
       = help: add `pub heigt: <type>` to the `Rectangle` struct definition
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
    error[E0277]: missing field `width` required by CGP component
      --> examples/src/base_area_2.rs:41:9
       |
      41 |         AreaCalculatorComponent,
         |         ^^^^^^^^^^^^^^^^^^^^^^^ unsatisfied trait bound
       |
       = help: struct `Rectangle` is either missing the field `width` or needs `#[derive(HasField)]`
       = note: this field is required by the trait bound `CanUseRectangle`
       = note: delegation chain:
               - required for `Rectangle` to implement `HasRectangleFields`
               - required for `RectangleArea` to implement the provider trait `AreaCalculator`
               - required for `Rectangle` to implement `the consumer trait `CanUseRectangle`
       = help: add `pub width: <type>` to the `Rectangle` struct definition or add `#[derive(HasField)]` if missing
    ");
}

#[test]
fn test_scaled_area_error() {
    let outputs = test_cgp_error_from_json("scaled_area.json", "scaled_area");

    // FIXME: the two source errors should be merged into one output.

    // We expect two error messages, but the first one should be suppressed (empty)
    // because it's a provider trait error that will be followed by a more detailed error
    assert_eq!(outputs.len(), 2, "Expected 2 error messages");

    // The first error should be empty (suppressed provider trait error)
    assert!(
        outputs[0].is_empty(),
        "First error should be suppressed (empty) since it's a redundant provider trait error"
    );

    // The second error should be the comprehensive CGP-formatted error
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

    assert_snapshot!(outputs[1], @"
    error[E0277]: missing field `height` required by CGP component
      --> examples/src/scaled_area.rs:58:9
       |
      58 |         AreaCalculatorComponent,
         |         ^^^^^^^^^^^^^^^^^^^^^^^ unsatisfied trait bound
       |
       = help: struct `Rectangle` is missing the field `height`
       = note: this field is required by the trait bound `CanUseRectangle`
       = note: delegation chain:
               - required for `Rectangle` to implement `HasRectangleFields`
               - required for `ScaledArea<RectangleArea>` to implement the provider trait `AreaCalculator`
               - required for `Rectangle` to implement `the consumer trait `CanUseRectangle`
       = help: add `pub height: <type>` to the `Rectangle` struct definition
    ");
}


#[test]
fn test_scaled_area_2_error() {
    let outputs = test_cgp_error_from_json("scaled_area_2.json", "scaled_area_2");

    assert_eq!(outputs.len(), 1, "Expected 1 error message");

    assert_snapshot!(outputs[0], @"
    error[E0277]: missing field `scalefactor` (possibly incomplete) required by CGP component
      --> examples/src/scaled_area_2.rs:58:9
       |
      58 |         AreaCalculatorComponent,
         |         ^^^^^^^^^^^^^^^^^^^^^^^ unsatisfied trait bound
       |
       = help: struct `Rectangle` is missing the field `scalefactor`
       = note: this field is required by the trait bound `CanUseRectangle`
       = note: delegation chain:
               - required for `Rectangle` to implement `HasScaleFactor`
               - required for `ScaledArea<RectangleArea>` to implement the provider trait `AreaCalculator`
               - required for `Rectangle` to implement `the consumer trait `CanUseRectangle`
       = help: add `pub scalefactor: <type>` to the `Rectangle` struct definition
    ");
}
