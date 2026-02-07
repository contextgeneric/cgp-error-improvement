# Show pending obligations as unsatisfied constraints in `report_similar_impl_candidates` #134348

Fixes: #134346

# Summary

This PR attempts to fix the issue in #134346, by additional `help` hints for each unsatisfied indirect constraint inside a blanket implementation.

# Details

To provide the extra information, a new variant `Select(PredicateObligations<'tcx>)` is added to `ScrubbedTraitError<'tcx>` to extract the `pending_obligations` returned from `select_where_possible`. Then inside `report_similar_impl_candidates`, we iterate through every pending obligation in the errors, and print them out as a `help` hint.

# Potential Issues

This is my first contribution to the Rust compiler project. I'm not sure of the best way to present the error messages, or handle them anywhere else in the codebase. If there are better ways to implement the improvement, I'm happy to modify the PR according to the suggestions.

An unresolved issue here is that the fix is only implemented for the old Rust trait solver. Compared to `OldSolverError`, I don't see the `pending_obligations` field to be present inside `NextSolverError`. So I'm unsure of the best way to keep this forward compatible with the upcoming trait solver.

I'm also not sure if the fix here would produce too noisy errors outside of my specific use case. When I test this fix with more complex projects that I have, the error messages may contain many unresolved constraints. However when scanning through all items, I determined that none of the listed unresolved constraints should be considered uninformative noise. Hence, I do think that it is essential to have all unresolved constraints listed in the error message.