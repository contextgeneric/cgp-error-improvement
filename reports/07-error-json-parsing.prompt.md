We are investigating the feasibility of building a cargo-cgp tool to better format CGP error messages.

You are given the example code scaled_area.rs, together with the human-readable error scaled_area.log, and the JSON error from `cargo check --message-format=json` in scaled_area.json.

Write down a hyper-detailed deep-dive investigation on how to parse and process the given JSON error message, so that we can present it as a cleaner CGP-specific error message to the end user.

Investigate whether it is feasible to use cargo-metadata to parse the error messages, or whether dedicated parser should be developed.

Provide implementation strategy on how we can write code that reconstruct the dependency graph and the involved constructs from the JSON error message.

Provide implementation strategy on how to identify redundant error messages, so that they are shown at most once.

Provide implementation strategy on whether it is possible to isolate the root cause of the error from the intermediary errors, so that we can present a more concise error message to the user.

Provide implementation strategy on how to identify CGP-specific constructs from the error messages, such as `HasField`, `DelegateComponent`, and `IsProviderFor`, so that we can hide the details and present them as native concepts to the user.

Show an example of how the final error message would look like after processing the JSON error, compared to the original human-readable error message.

Perform analysis on potential challenges or information that are missing from the JSON error, that might prevent the tool from gathering the necessary information to reconstruct the dependency graph.

Identify if all error information can be extracted from the structured JSON fields, or whether additional ad hoc parsing is required to process non-structured messages in the error. Analyze the impact on this on maintaining compatibility with future Rust versions.

Start your report by writing a summary, followed by a detailed table of content including all sub-sections. Before writing each chapter, start with a detailed outline of what you will write inside each section of that chapter.

Use full sentences and avoid point forms and tables.