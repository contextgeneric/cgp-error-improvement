# E0277: Unhelpful error message is given when indirect constraints cause blanket implementations to not get implemented #134346

### Code

```Rust
// The following code demonstrates an incomplete error message returned from
// the Rust compiler, when there are unsatisfied constraints present in
// code that makes use of _context-generic programming_, as described at
// https://patterns.contextgeneric.dev/.
//
// More details about similar error is described here:
// https://patterns.contextgeneric.dev/debugging-techniques.html


// The trait to link a consumer trait with a provide trait, as explained by:
// https://patterns.contextgeneric.dev/consumer-provider-link.html#blanket-consumer-trait-implementation
pub trait HasComponents {
    type Components;
}

// The trait to delegate a provider implementation to another provider, as explained by:
// https://patterns.contextgeneric.dev/provider-delegation.html#blanket-provider-implementation
pub trait DelegateComponent<Name> {
    type Delegate;
}

// The name of out example component
pub struct StringFormatterComponent;

// Our example consumer trait, with the context type being the implicit `Self` type
pub trait CanFormatToString {
    fn format_to_string(&self) -> String;
}

// Our example provider trait, with the context type being the explicit `Context` type
pub trait StringFormatter<Context> {
    fn format_to_string(context: &Context) -> String;
}

// A blanket implementation that links the consumer `CanFormatToString` with
// the provider `StringFormatter`, using `HasComponents`
impl<Context> CanFormatToString for Context
where
    Context: HasComponents,
    Context::Components: StringFormatter<Context>,
{
    fn format_to_string(&self) -> String {
        Context::Components::format_to_string(self)
    }
}

// A blanket implementation that links the provider implementation of
// `StringFormatter` with another provider, using `DelegateComponent`.
impl<Component, Context> StringFormatter<Context> for Component
where
    Component: DelegateComponent<StringFormatterComponent>,
    Component::Delegate: StringFormatter<Context>,
{
    fn format_to_string(context: &Context) -> String {
        Component::Delegate::format_to_string(context)
    }
}

// An example provider for `StringFormatter`, which has a generic
// implementation that requires `Context: debug`
pub struct FormatWithDebug;

impl<Context> StringFormatter<Context> for FormatWithDebug
where
    Context: core::fmt::Debug,
{
    fn format_to_string(context: &Context) -> String {
        format!("{:?}", context)
    }
}

// An example concrete context.
// Note: we pretend to forgot to derive `Debug` to cause error to be raised.
// FIXME: Uncomment the line below to fix the error.
// #[derive(Debug)]
pub struct Person {
    pub first_name: String,
    pub last_name: String,
}

// The components tied to the `Person` context
pub struct PersonComponents;

// Implement the consumer traits for `Person` using
// the aggregated provider `PersonComponents`
impl HasComponents for Person {
    type Components = PersonComponents;
}

// Implement `PersonComponents: StringFormatter<Person>` using `FormatWithDebug`
impl DelegateComponent<StringFormatterComponent> for PersonComponents {
    type Delegate = FormatWithDebug;
}

// Checks that `Person` implements `CanFormatToString`
pub trait CanUsePerson: CanFormatToString {}

// This should raise an error, since we didn't implement `Debug` for `Person`
impl CanUsePerson for Person {}
```

### Current output

```Shell
error[E0277]: the trait bound `FormatWithDebug: StringFormatter<Person>` is not satisfied
  --> lib.rs:99:23
   |
99 | impl CanUsePerson for Person {}
   |                       ^^^^^^ the trait `StringFormatter<Person>` is not implemented for `FormatWithDebug`, which is required by `Person: CanFormatToString`
   |
   = help: the trait `StringFormatter<Context>` is implemented for `FormatWithDebug`
note: required for `PersonComponents` to implement `StringFormatter<Person>`
  --> lib.rs:49:26
   |
49 | impl<Component, Context> StringFormatter<Context> for Component
   |                          ^^^^^^^^^^^^^^^^^^^^^^^^     ^^^^^^^^^
...
52 |     Component::Delegate: StringFormatter<Context>,
   |                          ------------------------ unsatisfied trait bound introduced here
note: required for `Person` to implement `CanFormatToString`
  --> lib.rs:37:15
   |
37 | impl<Context> CanFormatToString for Context
   |               ^^^^^^^^^^^^^^^^^     ^^^^^^^
...
40 |     Context::Components: StringFormatter<Context>,
   |                          ------------------------ unsatisfied trait bound introduced here
note: required by a bound in `CanUsePerson`
  --> lib.rs:96:25
   |
96 | pub trait CanUsePerson: CanFormatToString {}
   |                         ^^^^^^^^^^^^^^^^^ required by this bound in `CanUsePerson`

error: aborting due to 1 previous error

For more information about this error, try `rustc --explain E0277`.
```

### Desired output

```Shell
error[E0277]: the trait bound `FormatWithDebug: StringFormatter<Person>` is not satisfied
  --> lib.rs:99:23
   |
99 | impl CanUsePerson for Person {}
   |                       ^^^^^^ the trait `StringFormatter<Person>` is not implemented for `FormatWithDebug`
   |
   = help: the following constraint is not satisfied: `Person: Debug`
   = help: the trait `StringFormatter<Context>` is implemented for `FormatWithDebug`
note: required for `PersonComponents` to implement `StringFormatter<Person>`
  --> lib.rs:49:26
   |
49 | impl<Component, Context> StringFormatter<Context> for Component
   |                          ^^^^^^^^^^^^^^^^^^^^^^^^     ^^^^^^^^^
...
52 |     Component::Delegate: StringFormatter<Context>,
   |                          ------------------------ unsatisfied trait bound introduced here
note: required for `Person` to implement `CanFormatToString`
  --> lib.rs:37:15
   |
37 | impl<Context> CanFormatToString for Context
   |               ^^^^^^^^^^^^^^^^^     ^^^^^^^
...
40 |     Context::Components: StringFormatter<Context>,
   |                          ------------------------ unsatisfied trait bound introduced here
note: required by a bound in `CanUsePerson`
  --> lib.rs:96:25
   |
96 | pub trait CanUsePerson: CanFormatToString {}
   |                         ^^^^^^^^^^^^^^^^^ required by this bound in `CanUsePerson`

error: aborting due to 1 previous error

For more information about this error, try `rustc --explain E0277`.
```

### Rationale and extra context

# Summary

When resolving Rust constraints that involve blanket implementations, Rust does not produce helpful error messages when missing indirect constraints caused the blanket implementation to not get implemented.

# Rationale

The lack of informative error messages presents challenges in a new project that I am working on, [context-generic programming](https://www.contextgeneric.dev/), which implements a modular component system for Rust by making extensive use of blanket implementations. More details about how this error arised is described in the chapter of my book for [debugging techniques](https://patterns.contextgeneric.dev/debugging-techniques.html).

For the purpose of this issue, I have attached a minimal code snippet with desugared code, so that it can be tested without importing my library [`cgp`](https://crates.io/crates/cgp). For simplicity, the example code may look silly and does not demonstrate _why_ it is written that way. The main purpose of the example code is to reproduce an example error message with as little code as possible.

### Other cases

```Rust

```

### Rust Version

```Shell
rustc 1.83.0 (90b35a623 2024-11-26)
binary: rustc
commit-hash: 90b35a6239c3d8bdabc530a6a0816f7ff89a0aaf
commit-date: 2024-11-26
host: aarch64-unknown-linux-gnu
release: 1.83.0
LLVM version: 19.1.1
```

### Anything else?

The example code is also available on [Rust Playground](https://play.rust-lang.org/?version=stable&mode=debug&edition=2021&gist=6374d53722df90f7a7be5c7ae12fe333).

I already have a fix for this issue, and it is now available at #134348.