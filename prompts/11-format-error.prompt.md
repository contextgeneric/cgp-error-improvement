# Prompt

We will follow the directions in the given reports to build a new cargo-cgp crate that will intercept JSON output from cargo check with `--message-format=json` enabled, and display improved error messages for CGP code.

Currently, we have scaffolded a `render_compiler_message` function that formats a `CompilerMessage` into a human-readable string. The implementation currently simply return the `rendered` field from `Diagnostic`.

You are tasked to rewrite `render_compiler_message` to produce better error messages for CGP code.

When the error message does not involve the use of CGP, the code should simply return the original error message without any modifications.

For the initial work, focus only on improving the error message for `base_area.rs`, with the JSON output stored in `base_area.json`, and the original human-readable error message stored in `base_area.log`.

To test the implementation, you should write a unit test that reads directly from `base_area.json`, parse it and calls `render_compiler_message`, and then print the output using `println!`. You should then run the test using `cargo test -p cargo-cgp -- --nocapture` to check the output of the improved error message.

You should not test or verify your code by compiling and running the `cargo-cgp` binary. Instead, this work can be verified solely based on the unit test that processes the `base_area.json` fixture.

# Follow Up

The error message has improved, but the internal details of CGP is leaked. There is no need to mention the internal CGP traits like `IsProviderFor` and `CanUseComponent`. When the error says that the constraints are required for a provider to implement `IsProviderFor`, it really means that the constraint is required for the provider to implement the provider trait.

Similarly, when the error says that the constraints are required for a context to implement `CanUseComponent`, it really means that the constraints are required for the context to implement the consumer trait.

The name of the provider trait can typically be inferred from the component name by removing the "Component" suffix. So for example, if you see the component name `AreaCalculatorComponent`, you can mention "the provider trait `AreaCalculator`". If no `Component` suffix is found, you can instead mention "the provider trait for `AreaCalculatorComponent`" The consumer trait typically follows a different convention, so you can simply mention something like "the consumer trait for `AreaCalculatorComponent`".

Update the source code to reflect these changes. Also add inline comments to explain that we want to hide internal CGP traits whenever possible.