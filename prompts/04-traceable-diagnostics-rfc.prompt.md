Based on the given reports, write a hyper-detailed and in-depth RFC to propose for adding the `#[diagnostic::traceable]` attribute to the Rust compiler.

Your target audience are Rust compiler developers who are experienced with the Rust compiler internals and Rust language design, but are unfamiliar with CGP and the motivation behind this proposal. However, you should also keep in mind that the Rust compiler developers are likely not have enough time and interest to learn and understand everything about CGP. 

Therefore, you should keep the explanation of CGP-specific concepts as brief and high-level as possible, while still providing enough context and motivation for the proposal.

You should define a clear and concise semantics for the `#[diagnostic::traceable]` attribute, following the programming language design and compiler design principles of Rust. This includes where the attribute can be applied, what it does, and how it interacts with other language features and compiler components.

You should include detailed examples of the effects of `#[diagnostic::traceable]` with simple non-CGP vanilla Rust code snippets. Show how the error messages are improved with `#[diagnostic::traceable]` compared to without it, and explain the underlying reasons for the improvements. You should include the expected error messages before and after `#[diagnostic::traceable]` is applied, and explain how the attribute helps the compiler to generate more informative and actionable error messages.

You should explain how `#[diagnostic::traceable]` can help CGP to improve the error messages of unsatisfied dependencies in CGP code. In particular, you should demonstrate whether `#[diagnostic::traceable]` can help remove the need for the `IsProviderFor` trait, by including it in the trait bounds in the blanket implementation of the consumer trait and provider trait. You should also explain how `#[diagnostic::traceable]` can be used to improve the error messages of higher-order providers and deeply nested dependencies in CGP code.

Try not to mention the `IsProviderFor` trait or use it in the examples, as it is mainly a hack to workaround the current limitations of CGP error messages. If `#[diagnostic::traceable]` is introduced, there would not be any need for `IsProviderFor`, and the attribute can be directly applied to the blanket implementations of the consumer and provider traits and in the provider implementations.

You should also include motivation and examples of using `#[diagnostic::traceable]` in other contexts beyond CGP, such as in regular Rust code where it can help improve the error messages of trait bounds and type inference.

Finally, you should discuss the potential implementation details of `#[diagnostic::traceable]`, including how it can be integrated into the Rust compiler's existing diagnostic system, and any potential challenges or limitations that may arise from its implementation.

You should skip any historical context including the previous attempts and discussions to fix CGP error messages in the Rust compiler.

Start your RFC by writing a summary, followed by a detailed table of content including all sub-sections. Before writing each chapter, start with a detailed outline of what you will write inside each section of that chapter.

Use full sentences and avoid point forms and tables.