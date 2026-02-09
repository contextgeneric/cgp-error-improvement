We will follow the directions in the given reports to build a new cargo-cgp crate that will intercept JSON output from cargo check with `--message-format=json` enabled, and display improved error messages for CGP code.

We are currently working on improving the `render_compiler_message` function to produce better error messages for CGP code.

We will now focus on improving the error message for `scaled_area.rs`, with the accompanied `scaled_area.json` log. Currently, the error message given is as follows:

```
error[E0277]: the trait bound `RectangleArea: AreaCalculator<Rectangle>` is not satisfied                                                                        
  --> examples/src/scaled_area.rs:58:9
   = help: the trait `AreaCalculator<Rectangle>` is not implemented for `RectangleArea`
   = help: the trait `AreaCalculator<__Context__>` is implemented for `RectangleArea`
   = note: required for `ScaledArea<RectangleArea>` to implement `cgp::prelude::IsProviderFor<AreaCalculatorComponent, Rectangle>`
   = note: required for `Rectangle` to implement `cgp::prelude::CanUseComponent<AreaCalculatorComponent>`
   = note: required by a bound in `CanUseRectangle`

error[E0277]: missing field `height` required by CGP componentssages-example                                                                                     
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

For more information about this error, try `rustc --explain E0277`.s-example

error: could not compile `cgp-error-messages-example` (lib) due to 2 previous errors
Build failed
```

There are a couple of issues with the error message: 

- The first error message is not detected and formatted by the current implementation.
- There is a dangling `s-example` at the end of the error message.
- There is a redundant mentioning that `ScaledArea<RectangleArea>` and `RectangleArea` do not implement the provider trait `AreaCalculator`. In such case, the tool should deduce that `ScaledArea<RectangleArea>` is not implemented because of `RectangleArea`. 
    - When tracing the relationship, ensure that other parameters in `IsProviderFor`, e.g. the `Context` type and any additional generic parameters, matches for the relationship between the two providers to be established.

Additionally, also make the following improvements to the code:

- When the error `HasField` is not implemented is shown, it may also be caused by the context type not deriving `HasField`. This is demonstrated in the example `base_area_2.rs`. 
    - So when we are unsure, we should update the hint that the error is caused either by a missing field in the context, or that the context needs to derive `HasField`.
    - We may be able to gain more certainty from the additional `help` message from the compiler that says "but trait `HasField<...>` is implemented for it". This indicates that the context has other implementations of `HasFields`, so the cause is likely a missing field instead of a missing `#[derive(HasField)]`.
- Refactor the existing unit tests so that the test logic can be reused by new tests like the one you are going to add.

To test the implementation, you should write a unit test that reads directly from the relevant .json files, parse it and calls `render_compiler_message`, and then print the output using `println!`. You should then run the test using `cargo test -p cargo-cgp -- --nocapture` with the specific test name to check the output of the improved error message.

You should also test running `cargo-cgp` against `scaled_area.rs` by running `target/debug/cargo-cgp cgp check`. Ensure that the output is really improved, such as all CGP error messages are formatted properly, and that there is no more dangling output from the previous line.

You should also run existing unit tests and check if there is any change in output. If the output has changed, ensure that the error messages for existing test cases do not get worse. After your changes are finalized, update the relevant `assert_snapshot` using the appropriate `cargo insta` command.