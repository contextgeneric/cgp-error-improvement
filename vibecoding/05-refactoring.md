# Prompt

## Overview

We are following the directions in `10-combined-report.report.md` to build a cargo-cgp crate that will intercept JSON output from cargo check with `--message-format=json` enabled, and display improved error messages for CGP code.

In the task, you will refactor the code base to make it more generalized and ready to be extended to support more complex CGP errors.

## Generalized Error Handling

The current code base is hardcoded to handle specific error messages from `cgp-error-messages-example`. The tool must be generalized to be able to handle all kinds of CGP error messages. There must not be any hard code identification of module, fields, or constructs coming from the user-provided code. You should only pattern match based on constructs from the CGP libraries, and from the Rust standard libraries.

## Merging Errors

The code base currently handles the merging of errors poorly, by skipping the first error message based on some heuristics. Instead, the function should first iterate and gather information from all error messages, and build an internal database about the errors. If an existing entry is found in the internal database for an error message, then add update that entry to add the extra information provided by that error message. After processing all error messages, then walk through the internal database to build new set of error messages. The number of transformed messages do not need to match the number of original error messages.

## Restructuring based on report

Have a deep study through the report in `10-combined-report.report.md`, and compare the proposal to the organization in the current code base. Come up with a plan on how to refactor and improve the code base, by using appropriate suggestions from the report. Do not introduce any new features, and only focus on improving the current code.

## Testing

You should run existing unit tests and check if there is any change in output. If the output has changed, ensure that the error messages for existing test cases do not get worse. After your changes are finalized, update the relevant `assert_snapshot` using the appropriate `cargo insta` command.

## Planning

Before you start, write a detailed plan on the changes you are going to make in your response.