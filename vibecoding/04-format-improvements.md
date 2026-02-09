# Prompt

We will follow the directions in the given reports to build a new cargo-cgp crate that will intercept JSON output from cargo check with `--message-format=json` enabled, and display improved error messages for CGP code.

We are currently working on improving the `render_compiler_message` function to produce better error messages for CGP code.

We will now focus on improving the error message for `scaled_area.rs`, with the accompanied `scaled_area.json` log. Currently, the error message given is as follows:

```
error[E0277]: the trait bound `RectangleArea: AreaCalculator<Rectangle>` is not satisfied
  --> examples/src/scaled_area.rs:58:9
   = help: the trait `AreaCalculator<Rectangle>` is not implemented for `RectangleArea`
   = help: the trait `AreaCalculator<__Context__>` is implemented for `RectangleArea`
   = note: required for `ScaledArea<RectangleArea>` to implement `IsProviderFor<AreaCalculatorComponent, ...>`
   = note: required for `Rectangle` to implement `cgp::prelude::CanUseComponent<AreaCalculatorComponent>`
   = note: required by a bound in `CanUseRectangle`
   = note: the full name for the type has been written to '/home/soares/development/cgp-error-improvement/target/debug/deps/cgp_error_messages_example-8e13d63a65e6d8bf.long-type-13864714930861730567.txt'
   = note: consider using `--verbose` to print the full type name to the console

error[E0277]: missing field `height` required by CGP component
  --> examples/src/scaled_area.rs:58:9
   |
  58 |         AreaCalculatorComponent,
     |         ^^^^^^^^^^^^^^^^^^^^^^^ unsatisfied trait bound
   |
   = help: struct `Rectangle` is missing the field `height`
   = note: this field is required by the trait bound `HasRectangleFields`
   = note: delegation chain:
           - required for `Rectangle` to implement `HasRectangleFields`
           - required for `ScaledArea<RectangleArea>` to implement the provider trait `AreaCalculator`
           - required for `Rectangle` to implement the consumer trait for `AreaCalculatorComponent`
   = help: add `pub height: f64` to the `Rectangle` struct definition

For more information about this error, try `rustc --explain E0277`.

Build failed
```

The first error message contains help messages that are confusing:

- The message ``help: the trait `AreaCalculator<Rectangle>` is not implemented for `RectangleArea` `` is a repetition of the main message.
- The message ``help: the trait `AreaCalculator<__Context__>` is implemented for `RectangleArea` `` does not help on anything. It is only for cargo-cgp to know internally that this is likely a provider trait, since it has a blanket implementation.
- The message ``note: required for `ScaledArea<RectangleArea>` to implement `IsProviderFor<AreaCalculatorComponent, ...>` `` is only useful for cargo-cgp to know internally that there is some unsatisfied dependency to implement the provider trait.
- The message ``note: required for `Rectangle` to implement `cgp::prelude::CanUseComponent<AreaCalculatorComponent>` `` is only useful for cargo-cgp to knowinternally that the top-level error originate from trying to find an implementation of the consumer trait of `AreaCalculatorComponent` for the `Rectangle` context.
- The message ``note: required by a bound in `CanUseRectangle` `` gives cargo-cgp a hint that the name of the consumer trait is `CanUseRectangle`.

If we compare the first error message with the second error message, we can see that they are actually talking about the same error. Cargo-cgp should track the relation between multiple error messages, and combine them into one error message that shares the dependency graph when they are related.

You should update cargo-cgp use the `help` and `note` messages to help identify information such as that the error is related to the implementation of a CGP provider, and the name of the consumer trait.

When doing the refactoring, use any appropriate strategy that is proposed in `10-combined-report.report.md`.

To test the implementation, you should write or update a unit test that reads directly from the relevant .json files, parse it and calls `render_compiler_message`, and then print the output using `println!`. You should then run the test using `cargo test -p cargo-cgp -- --nocapture` with the specific test name to check the output of the improved error message.

You should also test running `cargo-cgp` against `scaled_area.rs` by running `target/debug/cargo-cgp cgp check`. Ensure that the output is really improved, such as all CGP error messages are formatted properly, and that there is no more dangling output from the previous line.

You should also run existing unit tests and check if there is any change in output. If the output has changed, ensure that the error messages for existing test cases do not get worse. After your changes are finalized, update the relevant `assert_snapshot` using the appropriate `cargo insta` command.

## Follow Up

You have skipped the first message, but you didn't reuse the information from the first error message to improve the second error message. For example, we know that the consumer trait for `AreaCalculatorComponent` is `CanUseRectangle` based on the information from the first error message.