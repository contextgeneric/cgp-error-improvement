# Prompt

## Overview

We are following the directions in `10-combined-report.report.md` to build a cargo-cgp crate that will intercept JSON output from cargo check with `--message-format=json` enabled, and display improved error messages for CGP code.

In the task, you will refactor the code base and make the following improvements.

## Field Errors

Currently, non-ASCII characters like `_` don't seem to get extracted when parsing `HasField`. For example, `test_scaled_area_2_error` shows the missing field as `scalefactor` instead of `scale_factor`.

In the help message, instead of asking the user to add a field like `pub height: <type>` with an unknown type, you should write a message that tells the user to ensure that a field of the appropriate type is present in the given context.

## Errors Related to missing `#[derive(HasField)]`

In the example `base_area_2.rs`, the `Rectangle` context contains the necessary fields but didn't include `#[derive(HasField)]` in its definition. But the transformed error message says that the field is missing, which is incorrect.

We can get a hint of whether a `HasField` error is due to missing field or missing `#[derive(HasField)]`, by looking at the `help` error messages. When a context has derived `HasField`, the error message would say something like: 

```
help: the trait `HasField<...>` is not implemented for `Context` 
but trait `HasField<...>` is implemented for it``
```

When the help message of that shape is not present, it is likely that the user forgot to derive `HasField` on the context, and we can inform the user with the appropriate message.

## Merging Errors

The code base sometimes handles the merging of errors poorly, returning output that contains empty messages like in `test_scaled_area_error`.

Cargo-cgp should first iterate and gather information from all error messages, and build an internal database about the errors. If an existing entry is found in the internal database for an error message, then add update that entry to add the extra information provided by that error message. After processing all error messages, then walk through the internal database to build new set of error messages. The number of transformed messages do not need to match the number of original error messages.

## Testing

You should run existing unit tests and check if there is any change in output. After your changes are finalized, update the relevant `assert_snapshot` using the appropriate `cargo insta` command.

If the output has changed, ensure that the error messages for existing test cases do not get worse.

## Planning

Before you start, write a detailed plan on the changes you are going to make in your response.