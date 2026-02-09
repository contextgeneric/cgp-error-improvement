Based on the given reports, explore whether it is feasible to build an external tools like `cargo cgp` as an alternative for improving error messages in CGP code.

The tool will be built in similar ways as Clippy and cargo-semver-checks. The main purpose of this tool would be to run the Cargo commands like `cargo check` via `cargo cgp check`, and the tool would intercept any error message produced and present them in a much more user-friendly and CGP-aware way.

Evaluate the trade offs between this external tool approach compared to modifying the Rust compiler, such as the ease of implementation, user experience, and interoperability strategy with the Rust compiler.

You should provide a detailed deep-dive walk through of how similar tools like Clippy and cargo-semver-checks are implemented, and how they integrate and interact with the Rust compiler.

After that, provide a detailed plan of which libraries and functions cargo-cgp would need to access in order to implement the desired functionality. This includes how to forward a call like `cargo cgp check` to perform `cargo check`, and how to intercept and process the error messages produced by `cargo check`.

You should also investigate the strategy on how to parse the raw error messages from the Rust compiler to be reformatted differently by cargo-cgp. In particular, investigate how should we ensure that the parsing of error message information is robust enough to not break with future changes in the Rust compiler's error message format.

Start your report by writing a summary, followed by a detailed table of content including all sub-sections. Before writing each chapter, start with a detailed outline of what you will write inside each section of that chapter.

Use full sentences and avoid point forms and tables.