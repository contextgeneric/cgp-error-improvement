# Implementing CGP Error Message Improvements in the Rust Compiler

## Executive Summary

This report provides a comprehensive analysis of the Rust compiler's trait solving and error reporting architecture, with specific focus on implementing improvements for Context-Generic Programming (CGP) error messages. The analysis reveals that the next-generation trait solver already contains sophisticated proof tree inspection facilities and obligation cause tracking mechanisms that can be leveraged for better error reporting. While the old solver uses obligation forests to track dependency chains, the new solver uses a proof tree visitor pattern with explicit derived cause construction that provides a cleaner foundation for implementing root cause identification.

The key finding is that the new solver's `BestObligation` visitor in derive_errors.rs already attempts to find leaf obligations by traversing proof trees, but it does not expose pending constraint information in a way that surfaces root causes clearly. The implementation strategy should focus on enhancing this visitor to collect and report all unsatisfied leaf constraints, implementing intelligent filtering based on dependency relationships, and providing mechanisms for library authors to mark constraints as requiring explicit reporting.

The old solver requires similar improvements but with different implementation approaches due to its use of obligation forests rather than proof trees. However, given the ongoing transition to the new solver, this report recommends focusing implementation effort primarily on the new solver while providing minimal compatibility patches for the old solver to prevent type errors during the transition period.

## Table of Contents

### Chapter 1: Architecture of the Next-Generation Trait Solver
- Understanding Proof Tree Construction and Goal Evaluation
- The Role of Canonicalization and State Management  
- How Candidates Are Selected and Nested Goals Generated
- The ProofTreeVisitor Pattern for Inspection
- Integration with the Fulfillment Engine

### Chapter 2: Error Derivation in the New Solver
- The `fulfillment_error_for_no_solution` Entry Point
- How `find_best_leaf_obligation` Traverses Proof Trees
- The `BestObligation` Visitor Implementation Line-by-Line
- Candidate Filtering and `non_trivial_candidates`
- Derived Cause Construction with `derive_cause` and `derive_host_cause`

### Chapter 3: Obligation Cause Tracking and Chain Construction
- The `ObligationCause` and `ObligationCauseCode` Architecture
- How `derived_cause` Builds Cause Chains
- The Role of `ImplDerivedCause` in Tracking Implementations
- Difference Between Trait-Derived and Host-Derived Causes
- Limitations of Current Cause Tracking for Deep CGP Chains

### Chapter 4: The Old Solver's Obligation Forest Architecture
- Understanding `ObligationForest` and Tree Structures
- How `PendingPredicateObligation` Tracks Dependencies
- The `FulfillmentContext` and Batch Processing
- Error Collection and the `to_errors` Method
- Why Root Causes Get Lost in Obligation Forests

### Chapter 5: Error Reporting Layer Analysis
- The `report_selection_error` Main Entry Point
- How `report_similar_impl_candidates` Works
- Filtering Heuristics in Error Message Generation
- The Gap Between Available Information and Reported Information
- Integration with `#[rustc_on_unimplemented]` Attribute

### Chapter 6: Implementation Strategy for New Solver Improvements
- Enhancing `BestObligation` to Collect All Leaf Constraints
- Building Dependency Graphs from Proof Tree Structure
- Root Cause Identification Algorithm Design
- Implementing the `#[diagnostic::traceable]` Attribute
- Specialized Formatting for CGP Patterns

### Chapter 7: Implementation Strategy for Old Solver Compatibility
- Minimal Changes to Maintain Type Compatibility
- Extracting Pending Obligations from Obligation Forests
- Adapting PR 134348 Approach for Completeness
- Migration Path and Deprecation Strategy
- Testing Strategy for Both Solvers

### Chapter 8: Detailed Implementation Roadmap
- Phase 1: Proof of Concept for Leaf Constraint Collection
- Phase 2: Dependency Graph Construction
- Phase 3: Traceable Attribute Integration
- Phase 4: Error Message Formatting Improvements
- Phase 5: Old Solver Compatibility Layer
- Phase 6: Testing, Documentation, and Stabilization

---

## Chapter 1: Architecture of the Next-Generation Trait Solver

### Chapter Outline

This chapter provides a comprehensive examination of how the next-generation trait solver is architected, focusing on the structures and mechanisms that will be leveraged for improved error reporting. We will explore how goals are evaluated through proof tree construction, how canonicalization enables sound caching and inference constraint management, how candidate selection drives the solving process, and how the proof tree visitor pattern provides a clean mechanism for post-hoc analysis of solving results. The chapter concludes by examining how the solver integrates with the fulfillment engine to drive batch obligation processing.

### Understanding Proof Tree Construction and Goal Evaluation

The next-generation trait solver operates on goals rather than obligations in its core algorithm. A goal, as defined in inspect.rs, represents a query about whether a particular predicate holds in a given parameter environment. Each goal is evaluated through a systematic process that explores possible proof strategies, represented as candidates, and recursively evaluates nested goals required by each candidate.

When the solver encounters a goal such as "does type Rectangle implement trait CanCalculateArea," it first examines what candidate implementations might satisfy this goal. For a trait goal, candidates include explicit implementations defined in code, blanket implementations from the trait definition, builtin implementations for language items, and bounds from the parameter environment. The solver systematically attempts each candidate, tracking which ones succeed, which ones fail, and which ones remain ambiguous.

Each candidate that the solver explores may generate nested goals representing constraints that must be satisfied for the candidate to be applicable. For example, when considering a blanket implementation with a where clause, the solver generates nested goals for each predicate in the where clause. These nested goals are themselves evaluated recursively, building a tree structure where the original goal is the root and leaf goals represent predicates that can be evaluated without further recursion.

The proof tree is not constructed eagerly during normal solving. Instead, the solver maintains enough information to reconstruct the proof tree later if needed. This design choice reflects performance considerations: constructing full proof trees for every goal would impose significant overhead, while the solver only needs detailed proof information when generating diagnostics or performing other post-hoc analysis. The reconstruction happens through the proof tree visitor mechanism, which we will examine later.

The evaluation result for each goal is represented by a `QueryResult` containing a `Certainty`. A goal can be certainly satisfied (Certainty::Yes), possibly satisfied pending further information (Certainty::Maybe with a cause indicating ambiguity or overflow), or definitely not satisfied (represented by an Err result containing NoSolution). This tristate result allows the solver to distinguish between definite failures requiring error reporting, permanent ambiguities that may resolve with more type information, and transient ambiguities due to solver limitations.

### The Role of Canonicalization and State Management

Canonicalization is a critical mechanism in the new solver that enables sound caching of goal evaluation results and proper handling of inference constraints. When a goal contains inference variables, the solver cannot cache its evaluation result directly because the result depends on what values those inference variables eventually receive. Canonicalization transforms goals with inference variables into canonical form by replacing the inference variables with placeholders, allowing the solver to cache results for the canonical form and later apply those results to concrete instantiations.

The `CanonicalState` type defined in inspect.rs wraps arbitrary data along with canonicalization information. When the solver evaluates a goal, it canonicalizes the goal to create a cache key, evaluates the canonical goal, and stores the result in canonical form. Later evaluations of the same goal (up to inference variable substitution) can retrieve the cached result and instantiate it with the current inference variable values.

This canonicalization mechanism has important implications for error reporting. When a goal fails, the failure information is stored in canonical form, which means inference variables in error messages have been replaced with placeholders. Error reporting must therefore carefully instantiate canonical errors back into the user's context to ensure error messages reference the actual types the user wrote rather than abstract placeholders.

State management in the new solver is organized around snapshots and probes. When the solver explores a candidate that might not work out, it creates a probe that allows rolling back inference constraints if the candidate fails. This rollback capability is essential for the solver to try multiple candidates without committing to inference decisions prematurely. However, it also means that by the time error reporting occurs, the solver has rolled back many exploration paths and only retained information about the path that produced the final failure.

### How Candidates Are Selected and Nested Goals Generated

Candidate selection in the new solver is driven by the type of goal being evaluated. For trait goals, the solver examines impl declarations, blanket impls, auto trait rules, builtin impls for compiler-intrinsic traits, and parameter environment bounds. For projection goals (normalizing associated types), the solver considers projection clauses from implementations and equalities from the parameter environment. For other goal types, specialized candidate generation logic applies.

Each candidate has an associated result indicating whether the candidate was found successful, ambiguous, or failing. When a candidate is selected as the best candidate (for example, because it was the only non-failing candidate), the solver generates nested goals based on that candidate's requirements. For an impl candidate, nested goals come from the impl's where clause predicates. For a builtin candidate, nested goals encode the builtin rules (for example, structural requirements for auto traits).

The nested goal generation process is critical for error reporting because it establishes the dependency relationships between goals. Each nested goal records its source through the `GoalSource` enum, which distinguishes between goals arising from impl where bounds, goals from well-formedness checking, goals from normalizing types, and other categories. This source information allows error reporting to understand why a particular goal was checked and thus to construct meaningful explanations of why a top-level goal failed.

When generating nested goals for an impl where bound, the solver creates derived obligation causes that link the nested goal back to the parent goal and the impl being applied. This linkage is essential for building the cause chain that appears in error messages. The `derive_cause` function in derive_errors.rs constructs these derived causes, embedding information about which impl was selected and which predicate from that impl generated the nested goal.

### The ProofTreeVisitor Pattern for Inspection

The proof tree visitor pattern is the new solver's mechanism for post-hoc analysis of solving results. After a goal has been evaluated, code can retrospectively examine how the goal was solved by visiting the proof tree. The `ProofTreeVisitor` trait defined in rustc_next_trait_solver/solve/inspect provides callbacks that are invoked as the visitor traverses the proof tree.

A visitor implementation provides a `visit_goal` method that is called for each goal in the tree. The method receives an `InspectGoal` providing access to the goal's predicate, result, and candidates. The visitor can examine this information and recursively visit nested goals by calling methods on the inspect goal object. The visitor returns a `ControlFlow` result allowing it to continue traversal or short-circuit and return early with a result.

The proof tree visitor pattern is used extensively for error derivation in the new solver. The `BestObligation` struct in derive_errors.rs implements `ProofTreeVisitor` to traverse proof trees and identify the best leaf obligation to report. This visitor starts at a root goal that failed, walks through the proof tree examining which candidates were attempted and which nested goals were generated, and ultimately selects a leaf goal to report as the primary error.

The visitor pattern's flexibility allows implementing different analysis strategies for different purposes. For error reporting, the `BestObligation` visitor prioritizes finding leaf goals that represent actual missing capabilities rather than transitive failures. For other purposes, different visitors could collect statistics about solving behavior, identify performance bottlenecks, or extract structured information about why solving succeeded or failed.

One limitation of the current visitor implementation is that it must reconstruct proof trees on demand, which means it cannot observe exploration paths that were abandoned due to failure. The solver only constructs proof trees for candidates that were seriously considered, not for candidates that were ruled out early by fast-path checks. This limitation means tha error reporting cannot always explain why certain implementations were not considered, only why the implementations that were considered did not work.

### Integration with the Fulfillment Engine

The fulfillment engine represents the layer above the trait solver that manages the full set of obligations needing resolution. While the trait solver focuses on evaluating individual goals, the fulfillment engine coordinates solving of multiple related obligations, handles ordering concerns, and manages the overall workflow of obligation processing during type checking.

The new solver's fulfillment context is implemented in rustc_trait_selection/src/solve/fulfill.rs. The `FulfillmentCtxt` struct maintains collections of pending obligations organized by type: regular predicate obligations, stalled obligations waiting on inference progress, and obligations that may be malformed. The fulfillment engine processes these obligations in waves, repeatedly attempting to make progress until no more obligations can be resolved.

When processing obligations, the fulfillment engine invokes the solver for each pending obligation. If the solver returns success (Certainty::Yes), the obligation is removed from the pending set. If the solver returns an error (NoSolution), the obligation is recorded as failed. If the solver returns Maybe, the obligation remains pending for future attempts. This iterative process continues until either all obligations are resolved, no progress can be made on remaining obligations, or the iteration limit is reached.

Integration between the fulfillment engine and error reporting happens through the `fulfillment_error_for_no_solution`, `fulfillment_error_for_stalled`, and `fulfillment_error_for_overflow` functions in derive_errors.rs. These functions are called when the fulfillment engine has determined that an obligation cannot be satisfied or has stalled indefinitely. They create `FulfillmentError` objects containing the obligation, an error code indicating the type of failure, and a reference to the root obligation that originally triggered the check.

The root obligation is critical for error reporting because it represents what the user actually wrote in their code, while the failing obligation might be many levels deep in implementation details. The new solver's fulfillment engine carefully tracks root obligations throughout the solving process, ensuring this information is available when constructing errors. This tracking is more explicit than in the old solver, where root obligation information could be lost as obligations were transformed during processing.

---

## Chapter 2: Error Derivation in the New Solver

### Chapter Outline

This chapter examines in detail how the new solver's error derivation system works, focusing on the mechanisms that identify which obligation should be reported when a complex goal fails. We will perform a line-by-line analysis of the `find_best_leaf_obligation` function and the `BestObligation` visitor, explaining how they navigate proof trees to identify root causes. The chapter covers candidate filtering strategies, the construction of derived obligation causes, and the detection of special error patterns that require customized handling.

### The `fulfillment_error_for_no_solution` Entry Point

The `fulfillment_error_for_no_solution` function at line 22 of derive_errors.rs serves as the primary entry point for constructing error objects when the solver determines a goal definitely cannot be satisfied. This function is called by the fulfillment engine after concluding that no amount of additional type inference will allow an obligation to succeed.

```rust
pub(super) fn fulfillment_error_for_no_solution<'tcx>(
    infcx: &InferCtxt<'tcx>,
    root_obligation: PredicateObligation<'tcx>,
) -> FulfillmentError<'tcx> {
    let obligation = find_best_leaf_obligation(infcx, &root_obligation, false);
```

The function immediately delegates to `find_best_leaf_obligation`, passing `consider_ambiguities: false` to indicate that it should look for definite failures rather than ambiguous goals. This delegation reflects the design principle that the actual obligation to report may differ from the root obligation that triggered the check. A high-level trait bound failure might be caused by a deeply nested missing field, and reporting the missing field is more helpful than reporting the high-level bound.

Following the leaf obligation identification, the function constructs an appropriate error code based on the predicate type:

```rust
let code = match obligation.predicate.kind().skip_binder() {
    ty::PredicateKind::Clause(ty::ClauseKind::Projection(_)) => {
        FulfillmentErrorCode::Project(
            MismatchedProjectionTypes { err: TypeError::Mismatch },
        )
    }
```

For projection predicates (associated type equalities), the error code indicates a projection type mismatch. The actual types involved in the mismatch would be extracted from the projection predicate during error message formatting. This error code distinction allows the error reporting layer to provide specialized messages for different kinds of predicate failures.

For `ConstArgHasType` predicates, special handling extracts the const parameter's actual type:

```rust
ty::PredicateKind::Clause(ty::ClauseKind::ConstArgHasType(ct, expected_ty)) => {
    let ct_ty = match ct.kind() {
        ty::ConstKind::Unevaluated(uv) => {
            infcx.tcx.type_of(uv.def).instantiate(infcx.tcx, uv.args)
        }
        ty::ConstKind::Param(param_ct) => {
            param_ct.find_const_ty_from_env(obligation.param_env)
        }
```

This type extraction enables error messages to explain that "constant has type X but was expected to have type Y" rather than just "ConstArgHasType constraint not satisfied." The distinction between different const kinds (unevaluated vs parameter vs value) reflects the complexity of how const generics are represented internally.

The default case for trait predicates, well-formedness predicates, and other clause kinds uses `FulfillmentErrorCode::Select(SelectionError::Unimplemented)`:

```rust
ty::PredicateKind::Clause(_)
| ty::PredicateKind::DynCompatible(_)
| ty::PredicateKind::Ambiguous => {
    FulfillmentErrorCode::Select(SelectionError::Unimplemented)
}
```

This generic "unimplemented" error will be refined during error message generation using information from the obligation cause chain and by examining available implementations. The generic code here serves as a placeholder that error reporting can specialize.

Finally, the function constructs the `FulfillmentError` with all three pieces of information:

```rust
FulfillmentError { obligation, code, root_obligation }
```

The `obligation` field contains the best leaf obligation identified for reporting. The `code` field contains the classified error type. The `root_obligation` field preserves the original top-level obligation, which error reporting uses to determine where to anchor the primary diagnostic span and how to structure the explanation.

### How `find_best_leaf_obligation` Traverses Proof Trees

The `find_best_leaf_obligation` function at line 160 implements the core algorithm for identifying which obligation provides the most helpful error message. The function begins by resolving inference variables in the obligation to ensure error messages reference concrete types rather than placeholders:

```rust
fn find_best_leaf_obligation<'tcx>(
    infcx: &InferCtxt<'tcx>,
    obligation: &PredicateObligation<'tcx>,
    consider_ambiguities: bool,
) -> PredicateObligation<'tcx> {
    let obligation = infcx.resolve_vars_if_possible(obligation.clone());
```

Resolution is important because during solving, obligations may contain inference variables that have since received concrete assignments. Showing resolved types in error messages improves clarity by presenting types as the user understands them rather than showing abstract inference variables.

The function then uses `fudge_inference_if_ok` to perform inference variable resolution in a way that does not commit the inference context:

```rust
let obligation = infcx
    .fudge_inference_if_ok(|| {
        infcx
            .visit_proof_tree(
                obligation.as_goal(),
                &mut BestObligation { obligation: obligation.clone(), consider_ambiguities },
            )
            .break_value()
            .ok_or(())
```

The `fudge_inference_if_ok` method creates a temporary scope where inference constraints can be generated during proof tree visiting without committing those constraints to the main inference context. This is necessary because proof tree visiting may instantiate generic bounds and unify types as it reconstructs the solving process, but these unifications should not affect the real inference state.

Inside the fudged scope, `visit_proof_tree` invokes the proof tree visitor on the obligation converted to a goal. The `BestObligation` visitor is initialized with the current obligation and a flag indicating whether to consider ambiguities. The visitor traverses the proof tree, updating its internal `obligation` field to progressively refine which obligation should be reported.

The visitor returns a `ControlFlow` where `Break(obligation)` indicates a leaf obligation was found, while `Continue(())` indicates traversal should continue. The `break_value()` extracts the obligation from a Break result, and `ok_or(())` converts the Option into a Result that `fudge_inference_if_ok` expects. If the visitor fails to find a better obligation (returns Continue), the original obligation is used as a fallback.

A subtle detail here is that the cause in the refined obligation may contain freshly instantiated inference variables from the fudged scope, which must be extracted separately:

```rust
.map(|(cause, o)| PredicateObligation { cause, ..o })
.unwrap_or(obligation);
```

This manual reconstruction ensures the cause is properly resolved even though the obligation itself comes from the fudged scope. The fallback to the original obligation ensures that if proof tree visitation somehow fails, error reporting can still proceed with the top-level obligation.

The function concludes by deeply normalizing the selected obligation:

```rust
deeply_normalize_for_diagnostics(infcx, obligation.param_env, obligation)
```

Deep normalization resolves any remaining associated types to their concrete forms, ensuring error messages show fully reduced types. This is particularly important for CGP where associated types may be used extensively in provider implementations, and seeing the concrete types helps users understand what went wrong.

### The `BestObligation` Visitor Implementation Line-by-Line

The `BestObligation` struct maintains state during proof tree traversal. The struct is simple, containing only the current best obligation and the ambiguity consideration flag:

```rust
struct BestObligation<'tcx> {
    obligation: PredicateObligation<'tcx>,
    consider_ambiguities: bool,
}
```

This minimal state reflects that the visitor's job is straightforward: find the deepest obligation that represents a real failure or ambiguity, not just a transitive consequence of other failures.

The `with_derived_obligation` helper allows temporarily replacing the current obligation during nested visitation:

```rust
fn with_derived_obligation(
    &mut self,
    derived_obligation: PredicateObligation<'tcx>,
    and_then: impl FnOnce(&mut Self) -> <Self as ProofTreeVisitor<'tcx>>::Result,
) -> <Self as ProofTreeVisitor<'tcx>>::Result {
    let old_obligation = std::mem::replace(&mut self.obligation, derived_obligation);
    let res = and_then(self);
    self.obligation = old_obligation;
    res
}
```

This pattern allows the visitor to explore nested goals with proper context tracking. When visiting a nested goal, the visitor temporarily replaces its current obligation with one representing the nested goal's predicate. If the nested visitation returns Break with a better obligation, that becomes the overall result. If it returns Continue, this visitation unwinds and the original obligation is restored.

The `non_trivial_candidates` method filters candidates to focus on those worth examining:

```rust
fn non_trivial_candidates<'a>(
    &self,
    goal: &'a inspect::InspectGoal<'a, 'tcx>,
) -> Vec<inspect::InspectCandidate<'a, 'tcx>> {
    let mut candidates = goal.candidates();
    match self.consider_ambiguities {
        true => {
            candidates.retain(|candidate| candidate.result().is_ok());
        }
        false => {
            candidates.retain(|c| !matches!(c.kind(), inspect::ProbeKind::RigidAlias { .. }));
```

When looking for errors (not ambiguities), the visitor first removes rigid alias candidates. These represent alias normalization attempts that the solver tries separately from main trait resolution. Including them would muddy the error picture by introducing technical details about normalization that users don't need to understand.

The visitor then further filters candidates if multiple remain:

```rust
if candidates.len() > 1 {
    candidates.retain(|candidate| {
        goal.infcx().probe(|_| {
            candidate.instantiate_nested_goals(self.span()).iter().any(
                |nested_goal| {
                    matches!(
                        nested_goal.source(),
                        GoalSource::ImplWhereBound
                            | GoalSource::AliasBoundConstCondition
                            | GoalSource::AliasWellFormed
                    ) && nested_goal.result().is_err()
                },
            )
        })
    });
}
```

This filtering retains only candidates that have at least one failing impl where bound or well-formedness requirement. The reasoning is that if multiple candidates were attempted but some failed for "boring" reasons (like alias normalization issues that don't reflect real user errors), we want to focus on candidates that failed due to actual missing trait implementations or unsatisfied constraints.

### Candidate Filtering and `non_trivial_candidates`

The `non_trivial_candidates` method embodies important heuristics about what makes an error report helpful. By filtering out rigid alias candidates, the visitor avoids reporting errors that arise purely from the solver's internal normalization mechanics. Users don't think in terms of whether associated types normalize through rigid or flexible paths; they think in terms of whether their types satisfy trait bounds.

The multi-candidate filtering addresses cases where a goal has multiple potential implementations but all fail. In such cases, reporting all failures would overwhelm the user. The heuristic assumes that if a candidate has no failing impl where bounds, its failure is likely technical rather than representing a real missing capability. By focusing on candidates with failing where bounds, the error report directs users to what they actually need to implement.

However, these heuristics have limitations. In complex CGP scenarios with deeply nested delegations, a candidate might fail because a transitive dependency's where bound is unsatisfied, and that where bound itself might be best understood by examining its own nested failures. The current filtering doesn't traverse this full depth; it only looks one level deep. This is where the proposed improvements will add value by building complete dependency graphs.

### Derived Cause Construction with `derive_cause` and `derive_host_cause`

The `derive_cause` function at line 575 constructs derived obligation causes that link nested obligations back to the implementations that generated them. This linkage is crucial for error messages to explain implementation selection:

```rust
fn derive_cause<'tcx>(
    tcx: TyCtxt<'tcx>,
    candidate_kind: inspect::ProbeKind<TyCtxt<'tcx>>,
    mut cause: ObligationCause<'tcx>,
    idx: usize,
    parent_trait_pred: ty::PolyTraitPredicate<'tcx>,
) -> ObligationCause<'tcx> {
    match candidate_kind {
        inspect::ProbeKind::TraitCandidate {
            source: CandidateSource::Impl(impl_def_id),
            result: _,
        } => {
```

When the candidate is an impl, the function retrieves the corresponding where clause predicate:

```rust
if let Some((_, span)) =
    tcx.predicates_of(impl_def_id).instantiate_identity(tcx).iter().nth(idx)
{
    cause = cause.derived_cause(parent_trait_pred, |derived| {
        ObligationCauseCode::ImplDerived(Box::new(traits::ImplDerivedCause {
            derived,
            impl_or_alias_def_id: impl_def_id,
            impl_def_predicate_index: Some(idx),
            span,
        }))
    })
}
```

This code creates an `ImplDerived` cause code that records which impl was selected, which predicate from that impl's where clause is being checked, and the span of that predicate. Error reporting uses this information to generate messages like "required because of the requirements on the impl of `Trait` for `Type`," providing context about why the nested obligation exists.

For builtin impl candidates, the derived cause is simpler:

```rust
inspect::ProbeKind::TraitCandidate {
    source: CandidateSource::BuiltinImpl(..),
    result: _,
} => {
    cause = cause.derived_cause(parent_trait_pred, ObligationCauseCode::BuiltinDerived);
}
```

Builtin impls don't have explicit where clauses in user code, so the cause simply indicates derivation from a builtin rule. This appears in error messages as "required by a builtin impl." The distinction helps users understand whether an error arises from code they wrote versus compiler-internal rules.

The`derive_host_cause` function handles const-context host effect predicates similarly but with specialized handling for const conditions:

```rust
fn derive_host_cause<'tcx>(
    tcx: TyCtxt<'tcx>,
    candidate_kind: inspect::ProbeKind<TyCtxt<'tcx>>,
    mut cause: ObligationCause<'tcx>,
    idx: usize,
    parent_host_pred: ty::Binder<'tcx, ty::HostEffectPredicate<'tcx>>,
) -> ObligationCause<'tcx> {
```

Host effects relate to const evaluation and the const trait system. The derivation includes both regular predicates and const conditions:

```rust
if let Some((_, span)) = tcx
    .predicates_of(impl_def_id)
    .instantiate_identity(tcx)
    .into_iter()
    .chain(tcx.const_conditions(impl_def_id).instantiate_identity(tcx).into_iter().map(
        |(trait_ref, span)| {
            (
                trait_ref.to_host_effect_clause(
                    tcx,
                    parent_host_pred.skip_binder().constness,
                ),
                span,
            )
        },
    ))
    .nth(idx)
```

This chaining ensures that const-specific requirements are tracked alongside regular trait bounds. The error reporting system can then explain const-context failures with appropriate const-specific messaging.

---

## Chapter 3: Obligation Cause Tracking and Chain Construction

### Chapter Outline

This chapter examines how the Rust compiler tracks why obligations exist through the `ObligationCause` and `ObligationCauseCode` types. We will explore how cause chains are built through the `derived_cause` method, how different cause code variants represent different origins of obligations, and why the current tracking approach has limitations for deep CGP dependency chains. This understanding is essential for designing improvements that don't lose critical provenance information.

### The `ObligationCause` and `ObligationCauseCode` Architecture

The `ObligationCause` struct defined at line 43 of mod.rs serves as the compiler's record of why an obligation was created. Every obligation carries its cause, ensuring that when errors occur, the compiler can explain the context that led to the obligation being checked:

```rust
pub struct ObligationCause<'tcx> {
    pub span: Span,
    pub body_id: LocalDefId,
    code: ObligationCauseCodeHandle<'tcx>,
}
```

The span indicates where in the source code the obligation originated, giving error reporting a location to anchor diagnostic messages. The body_id identifies which function or constant body context generated the obligation, which matters for region resolution and understanding regional constraints within closures and nested functions.

The `code` field, wrapped in an `ObligationCauseCodeHandle`, contains the detailed explanation of why the obligation exists. The handle is an optimization that avoids heap allocation for the common `Misc` case:

```rust
pub struct ObligationCauseCodeHandle<'tcx> {
    code: Option<Arc<ObligationCauseCode<'tcx>>>,
}
```

When the code is `Misc` (representing obligations without special provenance), the handle stores `None` rather than allocating an Arc. This optimization reflects that many obligations arise from straightforward type checking rules and don't need detailed cause tracking. For obligations with interesting provenance, the Arc allows sharing cause codes across multiple obligations efficiently.

The `ObligationCauseCode` enum contains variants for every distinct category of obligation origin. The enum has dozens of variants covering everything from method calls to pattern matching to closure capture. For CGP error reporting, the most relevant variants are those related to trait implementation and derivation:

```rust
pub enum ObligationCauseCode<'tcx> {
    Misc,
    WhereClause(DefId, Span),
    DerivedCause(Box<DerivedCause<'tcx>>),
    ImplDerived(Box<ImplDerivedCause<'tcx>>),
    // ... many other variants
}
```

The `WhereClause` variant records obligations arising directly from where clause requirements. The `DerivedCause` and `ImplDerived` variants, which we'll examine closely, represent obligations that stem from applying trait implementations and resolving derived bounds.

### How `derived_cause` Builds Cause Chains

The `derived_cause` method at line 108 of mod.rs creates derived causes that chain from parent obligations to child obligations:

```rust
pub fn derived_cause(
    mut self,
    parent_trait_pred: ty::PolyTraitPredicate<'tcx>,
    variant: impl FnOnce(DerivedCause<'tcx>) -> ObligationCauseCode<'tcx>,
) -> ObligationCause<'tcx> {
```

The method takes the current cause (self) and extends it by wrapping it in a new derived cause code. The `parent_trait_pred` parameter records what high-level trait predicate this obligation is helping to satisfy. The `variant` closure allows the caller to choose whether to create a generic `DerivedCause` or a more specific variant like `ImplDerived`.

The implementation constructs the derived cause:

```rust
self.code = variant(DerivedCause { parent_trait_pred, parent_code: self.code }).into();
self
```

This creates a nested structure where the new code contains the old code as its `parent_code` field. Error reporting can walk this chain backward to see the full provenance of an obligation. For example, if an obligation A spawned obligation B which spawned obligation C, then C's cause has B's cause as parent_code, which has A's cause as parent_code, forming a linked chain A → B → C.

The comment in the code expresses an important design decision:

```rust
/*!
 * Creates a cause for obligations that are derived from
 * `obligation` by a recursive search (e.g., for a builtin
 * bound, or eventually a `auto trait Foo`). If `obligation`
 * is itself a derived obligation, this is just a clone, but
 * otherwise we create a "derived obligation" cause so as to
 * keep track of the original root obligation for error
 * reporting.
 */
```

This indicates that derived causes serve specifically to maintain the connection to root obligations. Without derived causes, error reporting would only see the immediate failing obligation (e.g., "type T doesn't implement Clone") without understanding that this obligation arose because of a where clause requiring Clone in a blanket implementation being applied.

### The Role of `ImplDerivedCause` in Tracking Implementations

The `ImplDerivedCause` struct provides specialized tracking for obligations that arise from applying trait implementations:

```rust
pub struct ImplDerivedCause<'tcx> {
    pub derived: DerivedCause<'tcx>,
    pub impl_or_alias_def_id: DefId,
    pub impl_def_predicate_index: Option<usize>,
    pub span: Span,
}
```

The `impl_or_alias_def_id` identifies which implementation was applied. The `impl_def_predicate_index` records which specific predicate from the implementation's where clause generated this obligation, numbering the predicates in their declared order. The `span` indicates where in the implementation's source code that predicate appears, allowing error messages to highlight the relevent part of the impl.

This detailed tracking enables error messages like:

```
the trait `Foo` is not implemented for `T`
required for `S` to implement `Bar`
required by a bound in the where clause on the impl of `Bar` for `S`
note: required by a bound in this impl
  --> impl.rs:12:10
   |
12 | impl<T: Foo> Bar for S<T> {
   |            ^^^ required by this bound in `Bar`
```

The message connects the missing `Foo` implementation to the specific impl where the `Foo` bound appears, helping users understand why their code requires `Foo` to be implemented.

For CGP, this tracking is particularly valuable because CGP code extensively uses blanket implementations with complex where clauses. When a provider's where clause includes `Self: Has<FieldName>`, and the context doesn't provide that field, the impl derived cause allows error reporting to say "required by RectangleArea's requirement that Self implements HasField<height>," directly pointing users to what they need to fix.

However, the current impl derived cause tracking has a limitation: it only records the immediate impl application, not the full chain of impl applications that led to a nested obligation. If impl A's where clause requires trait B, and satisfying trait B involves applying impl C whose where clause requires trait D, the impl derived cause for the D obligation records impl C but doesn't capture that impl C was only invoked because of impl A. This lost information is a key problem for CGP where such chains are common.

### Difference Between Trait-Derived and Host-Derived Causes

The `DerivedHostCause` variant exists alongside `DerivedCause` to handle const context obligations:

```rust
pub struct DerivedHostCause<'tcx> {
    pub parent_host_pred: ty::Binder<'tcx, ty::HostEffectPredicate<'tcx>>,
    pub parent_code: ObligationCauseCodeHandle<'tcx>,
}
```

While structurally similar to `DerivedCause`, it stores a host effect predicate rather than a trait predicate. Host effects relate to whether code can be executed in const contexts, which has special rules and requirements distinct from regular trait bounds.

The separation between trait-derived and host-derived causes reflects that const checking operates somewhat independently of regular trait resolution. A type might implement a trait but not implement it in a way that's const-compatible. The derived cause separation allows error messages to distinguish between "doesn't implement the trait" and "doesn't implement the trait in a const-context."

For CGP error reporting, host-derived causes are less immediately relevant since CGP primarily uses regular traits rather than const traits. However, if CGP were extended to support const-generic contexts, the same issues with cause chain depth would apply to host-derived causes, requiring parallel solutions.

### Limitations of Current Cause Tracking for Deep CGP Chains

The fundamental limitation of current cause tracking is that the chain depth is bounded by how many times `derived_cause` is called during obligation processing. In the new solver's `BestObligation` visitor, derived causes are constructed when visiting nested goals from impl where bounds:

```rust
(ChildMode::Trait(parent_trait_pred), GoalSource::ImplWhereBound) => {
    obligation = make_obligation(derive_cause(
        tcx,
        candidate.kind(),
        self.obligation.cause.clone(),
        impl_where_bound_count,
        parent_trait_pred,
    ));
    impl_where_bound_count += 1;
}
```

This code creates one level of derived cause for each impl where bound goal visited. However, the visitor doesn't necessarily visit all levels of the proof tree. If the visitor decides that a particular level is the "best" obligation to report, it stops descending and returns that obligation. Any deeper levels that contributed to the failure are not represented in the returned obligation's cause chain.

For CGP with five or six levels of delegation, if the visitor stops at level three because it determined that level is most relevant, levels four through six are not captured in the cause chain. Error reporting then has no way to know about those deeper levels, resulting in incomplete error messages that don't explain the full context.

Additionally, the cause chain records the linear path through the proof tree that the visitor traversed, but proof trees can be more complex than linear chains. A goal might have multiple failing nested goals representing independent constraints that all need to be satisfied. The current cause chain structure can only represent one path, potentially hiding other relevant failures.

The proposed improvements must address these limitations by either augmenting the cause chain structure to represent richer tree topology or by building separate dependency graph structures that capture the full proof tree and making those available to error reporting alongside the traditional cause chains.

---

## Chapter 4: The Old Solver's Obligation Forest Architecture

### Chapter Outline

This chapter examines the old trait solver's use of obligation forests to manage pending obligations and track dependencies between them. We will explore how the forest data structure organizes obligations into trees, how obligations are processed in batches, how errors are collected when trees fail, and why this architecture makes it difficult to preserve root cause information. Understanding these mechanisms is essential for designing compatibility approaches that work with both solvers during the transition period.

### Understanding `ObligationForest` and Tree Structures

The obligation forest, defined in rustc_data_structures/src/obligation_forest/mod.rs, implements a data structure specifically designed for batch processing of interdependent obligations. The forest represents obligations as nodes in tree structures where each tree has a root obligation and potentially many descendant obligations representing sub-problems:

```rust
pub struct ObligationForest<O: ForestObligation> {
    /// The list of obligations. In between calls to `process_obligations`,
    /// this list only contains nodes in the `Pending` or `Waiting` state.
    ///
    /// `Pending` obligations are waiting to be processed, while `Waiting`
    /// obligations have been processed and are awaiting completion of their
    /// dependencies.
    nodes: Vec<Node<O>>,
```

The forest uses a vector to store all nodes, with parent-child relationships encoded through indices rather than pointers. Each node has a `parent` field containing the index of its parent node, allowing upward traversal to construct backtraces when errors occur. Nodes also track their current state (pending, waiting on dependencies, success, error, or unreachable).

When an obligation is registered with the forest, it becomes the root of a new tree:

```rust
pub fn register_obligation(&mut self, obligation: O) {
    let obligation_tree_id = ObligationTreeId(self.scratch.next_tree_id);
    self.scratch.next_tree_id += 1;
    self.nodes.push(Node::new(obligation_tree_id, None, obligation));
}
```

The tree ID uniquely identifies this obligation tree, and the None parent indicates this is a root node. As the obligation is processed, if it generates sub-obligations (represented by the processor returning `ProcessResult::Changed`), those sub-obligations become children in the tree:

```rust
ProcessResult::Changed(children) => {
    // ... code that adds children nodes ...
    for child in children {
        let child_index = self.nodes.len();
        self.nodes.push(Node::new(
            obligation_tree_id,
            Some(parent_index),
            child,
        ));
    }
}
```

The children nodes share the same obligation tree ID as their parent, maintaining the tree identity even as the tree grows. The parent index allows later code to walk upward from any node to construct the full backtrace to the root.

### How `PendingPredicateObligation` Tracks Dependencies

The old solver's fulfillment context uses `PendingPredicateObligation` as the obligation type stored in the forest:

```rust
pub struct PendingPredicateObligation<'tcx> {
    pub obligation: PredicateObligation<'tcx>,
    pub stalled_on: Vec<TyOrConstInferVar>,
}
```

The `obligation` field contains the actual predicate that needs checking. The `stalled_on` field records inference variables that this obligation depends on, allowing the fulfillment engine to recognize when an obligation cannot make progress because its types contain uninferred variables.

This stalling mechanism is important for batch processing. If an obligation is stalled on inference variables, attempting to process it again won't help until those variables receive concrete values. The fulfillment engine can skip stalled obligations during processing waves, improving efficiency by focusing on obligations that might make progress.

The forest obligation cache key for `PendingPredicateObligation` combines both the parameter environment and the predicate:

```rust
type CacheKey = ty::ParamEnvAnd<'tcx, ty::Predicate<'tcx>>;

fn as_cache_key(&self) -> Self::CacheKey {
    self.obligation.param_env.and(self.obligation.predicate)
}
```

This cache key allows the forest to deduplicate obligations. If the same predicate needs checking in the same parameter environment multiple times (which can happen with complex generic code), the forest only processes it once, improving performance. However, this deduplication can interfere with error reporting if multiple distinct root obligations lead to the same leaf obligation through different paths.

### The `FulfillmentContext` and Batch Processing

The old solver's `FulfillmentContext` defined in rustc_trait_selection/src/traits/fulfill.rs wraps the obligation forest and coordinates obligation processing:

```rust
pub struct FulfillmentContext<'tcx, E: 'tcx> {
    predicates: ObligationForest<PendingPredicateObligation<'tcx>>,
    usable_in_snapshot: usize,
    _errors: PhantomData<E>,
}
```

The `predicates` forest holds all registered obligations. The `usable_in_snapshot` field enforces that the context is only used in the snapshot where it was created, preventing subtle bugs from mixing obligations across inference context snapshots.

Batch processing happens in the `select` method:

```rust
fn select(&mut self, selcx: SelectionContext<'_, 'tcx>) -> Vec<E> {
    let span = debug_span!("select", obligation_forest_size = ?self.predicates.len());
    let _enter = span.enter();
    let infcx = selcx.infcx;

    let outcome: Outcome<_, _> =
        self.predicates.process_obligations(&mut FulfillProcessor { selcx });
```

The `process_obligations` method on the forest iterates through pending obligations, invoking the `FulfillProcessor` to attempt satisfying each one. For each obligation, the processor calls trait selection to determine whether the obligation can be satisfied immediately, needs to wait on sub-obligations, or has failed:

```rust
fn process_obligation(
    &mut self,
    obligation: &mut PendingPredicateObligation<'tcx>,
) -> ProcessResult<PendingPredicateObligation<'tcx>, Error> {
    match obligation.obligation.predicate.kind().skip_binder() {
        ty::PredicateKind::Clause(ty::ClauseKind::Trait(data)) => {
            self.process_trait_obligation(obligation, Binder::dummy(data))
        }
        // ... other predicate kinds ...
    }
}
```

The processor examines the predicate kind and dispatches to specialized handling for trait predicates, projection predicates, and other kinds. For trait predicates, it invokes trait selection to search for implementations. If selection succeeds with nested obligations, those become children in the forest. If selection fails, the error is returned and the obligation tree is marked as failed.

### Error Collection and the `to_errors` Method

When the forest determines that an obligation tree has failed (all its pending obligations have permanently failed), it collects errors through the `to_errors` method:

```rust
pub fn to_errors(&mut self, error_code: impl Fn(&O) -> E) -> Vec<Error<O, E>> {
    let mut errors = vec![];
    
    for node_index in 0..self.nodes.len() {
        if self.nodes[node_index].state.is_error() {
            let backtrace = self.error_node_to_backtrace(node_index);
            errors.push(Error {
                error: error_code(&self.nodes[node_index].obligation),
                backtrace,
            });
        }
    }
    
    errors
}
```

For each node in an error state, the method constructs a backtrace by walking upward through parent links to the root. The backtrace is a vector of obligations starting from the root and ending at the failing leaf. This backtrace structure allows error reporting to understand the dependency chain.

However, the backtrace construction has limitations. It captures the linear path from root to leaf but doesn't capture the full tree structure if multiple children of a Node failed for independent reasons. The backtrace only includes one path, potentially hiding other relevant failures that contributed to the overall tree failure.

Additionally, by the time `to_errors` is called, the forest has already performed various optimizations like compressing completed subtrees and reordering nodes. Some intermediate obligation nodes may have been removed from the forest if they were considered uninteresting. This optimization improves performance but makes it harder for error reporting to reconstruct the complete picture of what was checked.

### Why Root Causes Get Lost in Obligation Forests

The obligation forest's design prioritizes efficient batch processing and deduplication over preserving detailed failure information. When an obligation fails, the forest records that the obligation's tree is in error state, but it doesn't specifically mark which leaf obligation represents the root cause versus which obligations are transitive failures.

Error reporting must then analyze backtraces to infer root causes. The heuristic approaches used in the old solver's error reporting, as seen in fulfillment_errors.rs, attempt to identify root causes by examining obligation predicates and looking for patterns like missing trait implementations. However, these heuristics are imperfect.

For CGP, the problem is amplified because the backtrace might contain six levels of obligations:
1. Context doesn't implement consumer trait
2. Because provider doesn't implement provider trait
3. Because provider's where clause requires helper trait
4. Because helper trait's blanket impl requires field accessor trait
5. Because field accessor trait's blanket impl requires HasField
6. Because HasField is not implemented (the actual root cause)

The forest backtrace contains all six levels, but error reporting's heuristics might decide to report level 2 or 3 because those appear to be the main failure points, inadvertently hiding level 6 which is the actionable information the user needs.

The forest also doesn't preserve information about which obligations were attempted but ruled out early. If multiple candidate implementations were considered but rejected for different reasons, the forest only retains information about the final candidate that was seriously evaluated. This makes it impossible for error reporting to explain "we tried implementation A but it requires X which you don't have, and we tried implementation B but it requires Y which you also don't have."

---

## Chapter 5: Error Reporting Layer Analysis

### Chapter Outline

This chapter examines the error reporting layer that sits above both trait solvers and is responsible for formatting errors into messages users see. We will analyze the main entry point `report_selection_error`, examine how similar implementations are found and reported, understand the filtering heuristics that determine what information appears in messages, identify the information gap between what's available and what gets reported, and explore how the `#[rustc_on_unimplemented]` attribute system integrates with error reporting.

### The `report_selection_error` Main Entry Point

The `report_selection_error` method in fulfillment_errors.rs is the primary function that transforms a trait selection failure into a diagnostic message. The function signature reveals its inputs:

```rust
pub fn report_selection_error(
    &self,
    mut obligation: PredicateObligation<'tcx>,
    root_obligation: &PredicateObligation<'tcx>,
    error: &SelectionError<'tcx>,
) -> ErrorGuaranteed {
```

The `obligation` parameter is the specific obligation that failed (potentially a leaf obligation selected by `find_best_leaf_obligation`). The `root_obligation` is the top-level obligation that triggered the check. The `error` contains classified information about why the obligation failed.

The function begins by handling special cases like well-formedness violations discovered through HIR-based checking:

```rust
if let ObligationCauseCode::WellFormed(Some(wf_loc)) =
    root_obligation.cause.code().peel_derives()
    && !obligation.predicate.has_non_region_infer()
{
    if let Some(cause) = self
        .tcx
        .diagnostic_hir_wf_check((tcx.erase_and_anonymize_regions(obligation.predicate), *wf_loc))
    {
        obligation.cause = cause.clone();
        span = obligation.cause.span;
    }
}
```

This special case handles situations where well-formedness checking through the HIR can provide better diagnostics than trait solving alone. The HIR-based check might identify specific syntactic issues that led to the well-formedness violation, providing context that pure trait solving doesn't capture.

The function then dispatches based on the selection error type. For `SelectionError::Unimplemented`, which is the most common case, it proceeds to generate a detailed "trait not implemented" error. The implementation distinguishes between reporting the leaf obligation versus the root obligation based on heuristics:

```rust
let (main_trait_predicate, main_obligation) = if let ty::PredicateKind::Clause(
    ty::ClauseKind::Trait(root_pred)
) = root_obligation.predicate.kind().skip_binder()
    && !leaf_trait_predicate.self_ty().skip_binder().has_escaping_bound_vars()
    && !root_pred.self_ty().has_escaping_bound_vars()
    && (/* ... type similarity checks ... */)
    && leaf_trait_predicate.def_id() != root_pred.def_id()
    && !self.tcx.is_lang_item(root_pred.def_id(), LangItem::Unsize)
{
    (
        self.resolve_vars_if_possible(
            root_obligation.predicate.kind().rebind(root_pred),
        ),
        root_obligation,
    )
} else {
    (leaf_trait_predicate, &obligation)
};
```

This heuristic attempts to identify cases where the root obligation provides a better error message than the leaf. The conditions check whether the root and leaf predicates have similar self types (suggesting they're related rather than being distinct problems) and whether the traits are different (indicating that the root is a higher-level abstraction worth reporting). 

For CGP, this heuristic can be problematic. The root obligation might be "Rectangle implements CanUseComponent for AreaCalculatorComponent" which is abstract, while the leaf might be "Rectangle implements HasField for height" which is concrete and actionable. The heuristic might choose the root because it appears at a higher level, hiding the concrete fix.

### How `report_similar_impl_candidates` Works

The `report_similar_impl_candidates` function searches for trait implementations that almost satisfy the obligation but fail due to unsatisfied constraints. This search provides the "the following impls exist but their where clauses aren't satisfied" section of error messages:

```rust
pub fn report_similar_impl_candidates(
    &self,
    trait_predicate: ty::PolyTraitPredicate<'tcx>,
    obligation_param_env: ty::ParamEnv<'tcx>,
    body_def_id: LocalDefId,
    err: &mut Diag<'_>,
    required_trait: Option<DefId>,
) -> bool {
```

The function iterates through all implementations of the trait:

```rust
let candidates = self.tcx.all_impls(trait_predicate.def_id())
    .filter_map(|def_id| {
        // Check if implementation matches the self type
        let impl_header = self.tcx.impl_trait_header(def_id)?;
        // Try to unify the impl's self type with the obligation's self type
        // ...
    })
    .collect();
```

For each potentially matching implementation, the function creates a fulfillment context and attempts to satisfy the impl's where clause predicates:

```rust
let ocx = ObligationCtxt::new(infcx);
ocx.register_obligations(obligations);
let errors = ocx.select_all_or_error();
```

If the where clause check produces errors, those errors indicate which constraints are preventing the implementation from being applicable. The function formats these errors into help messages explaining what's missing.

However, the current implementation has a limitation relevant to CGP: it only reports whether the implementation's where clause as a whole is satisfied or not, without drilling into why nested constraints within the where clause fail. If a where clause includes a bound like `Self: HasRectangleFields` that itself expands through blanket implementations requiring `HasField`, the function doesn't explain the `HasField` requirement. It only reports that `HasRectangleFields` is not satisfied, which is less helpful.

### Filtering Heuristics in Error Message Generation

Error message generation applies multiple levels of filtering to avoid overwhelming users. The first level is the selection of which obligation to report, as discussed above. The second level is the decision of which similar implementations to mention.

The similar implementation finding code limits how many implementations are reported:

```rust
candidates.sort_by_key(|&(a, b)| (Reverse(a), b));
candidates.dedup_by_key(|&mut (sim, _)| sim);
let len = candidates.len();
if len > 5 {
    candidates = candidates.into_iter().take(4).collect();
}
```

This code prioritizes more similar implementations (those requiring fewer additional bounds) and caps the number reported at 5, with a note about additional candidates if more exist. The cap prevents error messages from becoming pages long when many implementations exist.

The filtering also applies to which constraints from a where clause are mentioned. If a where clause has five predicates and three of them are unsatisfied, the current code doesn't clearly separate the unsatisfied constraints from the satisfied ones. The error might mention all five predicates or only the first unsatisfied one, depending on processing order and heuristics.

For CGP, this filtering becomes problematic when a provider has multiple constraints and only one is unsatisfied. The error message might list all constraints, leaving the user to figure out which one is the problem, or it might skip the constraints entirely and just say the provider doesn't work, providing no actionable information.

### The Gap Between Available Information and Reported Information

The fundamental gap is that while the compiler's internal data structures contain complete information about what was checked and what failed, error reporting only accesses a filtered subset of that information. The `fulfillment_error_for_no_solution` function identifies the best leaf obligation, but it doesn't collect information about the other branches of the proof tree that also failed. The similar implementation finding code tests whether where clauses are satisfied, but it doesn't extract and report the specific unsatisfied predicates.

This gap exists partly for good reasons: presenting all available information would create overwhelming error messages. But the current filtering is too aggressive for CGP patterns. A middle ground is needed where root cause predicates are always reported explicitly, even if that makes messages somewhat longer, while truly redundant in formation remains filtered.

The gap could be bridged by augmenting the error reporting API to include optional detailed diagnostic information that only appears when errors involve deep dependency chains or when users explicitly request verbose output. Currently, error reporting is binary: information is either included or excluded, with no middle ground for conditional inclusion based on code patterns.

### Integration with `#[rustc_on_unimplemented]` Attribute

The `#[rustc_on_unimplemented]` attribute allows trait authors to customize error messages when their trait is not implemented. The error reporting layer calls `on_unimplemented_note` to check whether the failing trait has customization:

```rust
let OnUnimplementedNote {
    message,
    label,
    notes,
    parent_label,
    append_const_msg,
} = self.on_unimplemented_note(main_trait_predicate, main_obligation, &mut long_ty_file);
```

If the trait has an `#[rustc_on_unimplemented]` attribute, the returned note contains customized message strings that replace the default "trait not implemented" message. This mechanism allows library authors to provide domain-specific guidance.

For CGP, the `IsProviderFor` trait likely includes `#[rustc_on_unimplemented]` annotations to ensure errors mention providers and components rather than using generic trait terminology. However, the attribute can only customize messages for the trait it's attached to, not for the entire cause chain. If a nested constraint from a different trait is the root cause, that trait's `#[rustc_on_unimplemented]` attribute (if any) determines the message, potentially breaking the CGP-specific terminology.

The attribute system also doesn't provide a way for traits to mark certain where clause predicates as requiring explicit reporting. The proposed `#[diagnostic::traceable]` attribute would fill this gap, giving trait authors control over which constraints should never be filtered from error messages.

---

## Chapter 6: Implementation Strategy for New Solver Improvements

### Chapter Outline

This chapter presents a detailed implementation strategy for improving error reporting in the next-generation trait solver. We will design enhancements to the `BestObligation` visitor to collect comprehensive leaf constraint information, develop algorithms for building dependency graphs from proof tree structures, create root cause identification logic, specify the implementation of the `#[diagnostic::traceable]` attribute interface, and design specialized formatting for CGP patterns. Each section provides specific code-level changes with rationale.

### Enhancing `BestObligation` to Collect All Leaf Constraints

The current `BestObligation` visitor returns a single obligation representing the best leaf to report. The enhanced version should collect all leaf constraints while still identifying the primary obligation to report. This allows error reporting to present the primary failure prominently while also listing related constraints that users need to address.

The enhanced visitor struct should maintain a collection of leaf failures:

```rust
struct BestObligation<'tcx> {
    obligation: PredicateObligation<'tcx>,
    consider_ambiguities: bool,
    // New field: collected leaf constraints
    leaf_constraints: Vec<LeafConstraint<'tcx>>,
}

struct LeafConstraint<'tcx> {
    predicate: ty::Predicate<'tcx>,
    span: Span,
    // Track the path from root to this leaf
    cause_chain: Vec<ObligationCauseCode<'tcx>>,
    // Whether this is from an impl where bound
    source: GoalSource,
}
```

The `LeafConstraint` structure captures not just the predicate that failed, but its location, the cause chain leading to it, and its source category. This rich information enables error reporting to distinguish between different types of leaf failures and format them appropriately.

The visitor should collect leaf constraints when it encounters goals that cannot be further refined:

```rust
fn visit_goal(&mut self, goal: &inspect::InspectGoal<'_, 'tcx>) -> Self::Result {
    // ... existing filtering logic ...
    
    let candidates = self.non_trivial_candidates(goal);
    let candidate = match candidates.as_slice() {
        [candidate] => candidate,
        [] => {
            // This is a leaf goal with no applicable candidates
            self.collect_leaf_constraint(goal);
            return self.detect_error_from_empty_candidates(goal);
        }
        _ => return ControlFlow::Break(self.obligation.clone()),
    };
    
    // ... rest of existing logic ...
}
```

The `collect_leaf_constraint` method would extract the constraint information:

```rust
fn collect_leaf_constraint(&mut self, goal: &inspect::InspectGoal<'_, 'tcx>) {
    let predicate = goal.goal().predicate;
    let span = self.obligation.cause.span;
    
    // Build the cause chain by extracting codes from current obligation
    let mut cause_chain = Vec::new();
    let mut current_code = self.obligation.cause.code();
    while !matches!(current_code, ObligationCauseCode::Misc) {
        cause_chain.push(current_code.clone());
        current_code = match current_code {
            ObligationCauseCode::ImplDerived(inner) => &inner.derived.parent_code,
            ObligationCauseCode::BuiltinDerived(inner) => &inner.parent_code,
            _ => break,
        };
    }
    
    self.leaf_constraints.push(LeafConstraint {
        predicate: predicate.upcast(goal.infcx().tcx),
        span,
        cause_chain,
        source: GoalSource::Misc, // Would be extracted from context
    });
}
```

This collection happens as the visitor traverses, building a complete picture of all leaf failures encountered during the search for the best primary obligation. The leaf constraint collection shouldn't change which obligation is selected as primary; it simply augments the result with additional context.

### Building Dependency Graphs from Proof Tree Structure

A dependency graph makes explicit the relationships between failed obligations, allowing error reporting to identify which failures are root causes versus which are consequences. The graph representation should be built after proof tree visitation completes:

```rust
struct DependencyGraph<'tcx> {
    nodes: FxIndexMap<ty::Predicate<'tcx>, NodeInfo<'tcx>>,
    edges: Vec<DependencyEdge<'tcx>>,
}

struct NodeInfo<'tcx> {
    predicate: ty::Predicate<'tcx>,
    // Whether this obligation failed
    failed: bool,
    // Whether this is a leaf (no dependencies on other pending obligations)
    is_leaf: bool,
    span: Span,
}

struct DependencyEdge<'tcx> {
    from: ty::Predicate<'tcx>,
    to: ty::Predicate<'tcx>,
    // The kind of dependency relationship
    kind: DependencyKind,
}

enum DependencyKind {
    ImplWhereClause { impl_def_id: DefId, predicate_index: usize },
    TraitSuper,
    WellFormedness,
    Projection,
}
```

The graph construction algorithm processes the collected leaf constraints and infers edges from the cause chains:

```rust
fn build_dependency_graph<'tcx>(
    primary_obligation: &PredicateObligation<'tcx>,
    leaf_constraints: &[LeafConstraint<'tcx>],
) -> DependencyGraph<'tcx> {
    let mut graph = DependencyGraph {
        nodes: FxIndexMap::default(),
        edges: Vec::new(),
    };
    
    // Add the primary obligation as a node
    graph.nodes.insert(
        primary_obligation.predicate,
        NodeInfo {
            predicate: primary_obligation.predicate,
            failed: true,
            is_leaf: false,
            span: primary_obligation.cause.span,
        },
    );
    
    // Process each leaf constraint
    for leaf in leaf_constraints {
        // Add leaf as a node
        graph.nodes.insert(
            leaf.predicate,
            NodeInfo {
                predicate: leaf.predicate,
                failed: true,
                is_leaf: true,
                span: leaf.span,
            },
        );
        
        // Extract dependencies from cause chain
        for code in &leaf.cause_chain {
            match code {
                ObligationCauseCode::ImplDerived(inner) => {
                    let from_pred = inner.derived.parent_trait_pred.upcast();
                    graph.ensure_node(from_pred);
                    graph.edges.push(DependencyEdge {
                        from: from_pred,
                        to: leaf.predicate,
                        kind: DependencyKind::ImplWhereClause {
                            impl_def_id: inner.impl_or_alias_def_id,
                            predicate_index: inner.impl_def_predicate_index.unwrap_or(0),
                        },
                    });
                }
                // Handle other cause code types...
                _ => {}
            }
        }
    }
    
    graph
}
```

The graph construction extracts dependency information that was implicit in cause chains and makes it explicit through edges. This enables graph algorithms to compute properties like:
- Which nodes are roots (no incoming edges)
- Which nodes are leaves (no outgoing edges)
- Which nodes are on the critical path from a root to a leaf
- Whether multiple independent roots exist requiring separate fixes

### Root Cause Identification Algorithm Design

Given a dependency graph, root cause identification proceeds by finding leaf nodes (obligations with no outgoing edges) that represent actual missing capabilities rather than transitive failures. The algorithm should distinguish between different categories of leaves:

```rust
enum RootCause<'tcx> {
    // A trait is not implemented at all
    MissingImpl {
        trait_ref: ty::TraitRef<'tcx>,
        self_ty: Ty<'tcx>,
    },
    // A field is missing from a struct
    MissingField {
        struct_ty: Ty<'tcx>,
        field_name: Symbol,
    },
    // A where clause bound is unsatisfied
    UnsatisfiedBound {
        bound: ty::Predicate<'tcx>,
        required_by: DefId,
    },
    // Multiple independent root causes exist
    Multiple(Vec<RootCause<'tcx>>),
}
```

The identification algorithm analyzes leaf nodes to classify them:

```rust
fn identify_root_causes<'tcx>(
    graph: &DependencyGraph<'tcx>,
    tcx: TyCtxt<'tcx>,
) -> Vec<RootCause<'tcx>> {
    let mut roots = Vec::new();
    
    for (predicate, info) in &graph.nodes {
        if !info.is_leaf {
            continue;
        }
        
        let root = classify_root_cause(*predicate, graph, tcx);
        roots.push(root);
    }
    
    // De-duplicate similar root causes
    deduplicate_roots(&mut roots);
    
    // If multiple roots exist, determine if they're independent or related
    if roots.len() > 1 {
        vec![RootCause::Multiple(roots)]
    } else {
        roots
    }
}

fn classify_root_cause<'tcx>(
    predicate: ty::Predicate<'tcx>,
    graph: &DependencyGraph<'tcx>,
    tcx: TyCtxt<'tcx>,
) -> RootCause<'tcx> {
    match predicate.kind().skip_binder() {
        ty::PredicateKind::Clause(ty::ClauseKind::Trait(trait_pred)) => {
            // Check if this looks like a HasField predicate
            if is_has_field_trait(trait_pred.def_id(), tcx) {
                let field_name = extract_field_name_from_trait_args(trait_pred.trait_ref);
                RootCause::MissingField {
                    struct_ty: trait_pred.self_ty(),
                    field_name,
                }
            } else {
                RootCause::MissingImpl {
                    trait_ref: trait_pred.trait_ref,
                    self_ty: trait_pred.self_ty(),
                }
            }
        }
        _ => {
            // For other predicate kinds, extract bound information
            RootCause::UnsatisfiedBound {
                bound: predicate,
                required_by: find_requiring_impl(predicate, graph),
            }
        }
    }
}
```

The classification uses CGP-specific knowledge (like recognizing `HasField` traits) to provide specialized handling for common patterns. This CGP awareness can be generalized: the classifier could check attributes on traits to determine if special formatting applies.

### Implementing the `#[diagnostic::traceable]` Attribute

The `#[diagnostic::traceable]` attribute should mark trait bounds as requiring explicit reporting. The attribute would be recognized by the compiler during attribute parsing and stored as part of the trait or bound's metadata:

```rust
// In rustc_middle/src/ty/trait_def.rs or similar
pub struct TraitDef {
    // ... existing fields ...
    
    /// Whether this trait's bounds should never be filtered from error messages
    pub traceable_bounds: bool,
}
```

The attribute syntax would follow Rust's diagnostic attribute conventions:

```rust
#[diagnostic::traceable]
trait IsProviderFor<Component, Context> {
    // ...
}
```

Or applied to specific bounds:

```rust
impl<T> SomeTrait for T
where
    #[diagnostic::traceable]
    T: ImportantBound,
    T: LessImportantBound,
{
    // ...
}
```

The compiler would check for this attribute when processing trait definitions:

```rust
fn check_trait_def(tcx: TyCtxt<'_>, def_id: LocalDefId) {
    let attrs = tcx.get_attrs(def_id.to_def_id());
    let traceable = attrs.iter().any(|attr| {
        attr.has_name(sym::diagnostic)
            && attr.meta_item_list().is_some_and(|list| {
                list.iter().any(|item| item.has_name(sym::traceable))
            })
    });
    
    if traceable {
        // Mark the trait definition as having traceable bounds
        // This information would flow through to error reporting
    }
}
```

Error reporting would check this flag when considering whether to filter a predicate:

```rust
fn should_report_predicate<'tcx>(
    predicate: ty::Predicate<'tcx>,
    tcx: TyCtxt<'tcx>,
) -> bool {
    match predicate.kind().skip_binder() {
        ty::PredicateKind::Clause(ty::ClauseKind::Trait(trait_pred)) => {
            let trait_def = tcx.trait_def(trait_pred.def_id());
            if trait_def.traceable_bounds {
                return true;
            }
            
            // Otherwise apply normal filtering heuristics
            apply_filtering_heuristics(predicate)
        }
        _ => apply_filtering_heuristics(predicate),
    }
}
```

The CGP macros would automatically add `#[diagnostic::traceable]` to `IsProviderFor` implementations, ensuring all provider constraints are explicitly reported.

### Specialized Formatting for CGP Patterns

CGP-specific error formatting should recognize CGP traits and format them using CGP terminology rather than generic trait language. The formatter would check trait def_ids against known CGP traits:

```rust
fn format_cgp_error<'tcx>(
    trait_ref: ty::TraitRef<'tcx>,
    tcx: TyCtxt<'tcx>,
) -> Option<String> {
    // Check if this is an IsProviderFor trait
    if is_is_provider_for_trait(trait_ref.def_id, tcx) {
        let component = extract_component_arg(trait_ref);
        let context = trait_ref.self_ty();
        let provider = extract_provider_arg(trait_ref);
        
        return Some(format!(
            "provider `{}` cannot implement component `{}` for context `{}`",
            provider,
            component,
            context
        ));
    }
    
    // Check if this is a DelegateComponent trait
    if is_delegate_component_trait(trait_ref.def_id, tcx) {
        let component = extract_component_arg(trait_ref);
        let context = trait_ref.self_ty();
        
        return Some(format!(
            "context `{}` does not delegate component `{}`",
            context,
            component
        ));
    }
    
    None
}
```

The specialized formatting would also handle type-level constructs used by CGP:

```rust
fn format_symbol_ty<'tcx>(ty: Ty<'tcx>, tcx: TyCtxt<'tcx>) -> Option<String> {
    // Recognize Symbol<...> type and extract the string
    if let ty::Adt(adt_def, args) = ty.kind() {
        if is_symbol_adt(adt_def.did(), tcx) {
            return Some(extract_symbol_string(args, tcx));
        }
    }
    None
}

fn extract_symbol_string<'tcx>(args: &[GenericArg<'tcx>], tcx: TyCtxt<'tcx>) -> String {
    // Walk through the nested Chars types and reconstruct the string
    let mut result = String::new();
    // ... implementation details ...
    result
}
```

This symbol extraction allows error messages to show `HasField<"height">` instead of `HasField<Symbol<5, Chars<'h', Chars<'e', ...>>>>`, dramatically improving readability.

---

## Chapter 7: Implementation Strategy for Old Solver Compatibility

### Chapter Outline

This chapter addresses the pragmatic need to maintain some level of error reporting improvement in the old solver during the transition to the new solver. We will design minimal changes that preserve type compatibility without requiring extensive old solver modifications, discuss adapting PR 134348's approach with completeness improvements, outline the migration path and deprecation strategy, and specify testing approaches that cover both solvers.

### Minimal Changes to Maintain Type Compatibility

The old solver's primary role during the transition period is to continue functioning without regressions while the new solver matures. From an error reporting perspective, this means we should avoid making the old solver's errors worse, but we don't need to achieve feature parity with new solver improvements.

The minimal compatibility strategy involves:

1. Ensuring that error types returned by the old solver remain compatible with error reporting infrastructure changes made for the new solver
2. Not introducing new error reporting features that would create different behavior between solvers
3. Providing stubs for new error reporting APIs that old solver error reporting can call but which may not provide enhanced information

The error type compatibility is straightforward because both solvers ultimately produce `FulfillmentError`:

```rust
pub struct FulfillmentError<'tcx> {
    pub obligation: PredicateObligation<'tcx>,
    pub code: FulfillmentErrorCode<'tcx>,
    pub root_obligation: PredicateObligation<'tcx>,
}
```

As long as the old solver continues populating these fields appropriately, error reporting code can handle errors from either solver uniformly. The key is ensuring that `root_obligation` is always set correctly in the old solver, as this field is critical for anchoring error messages.

### Extracting Pending Obligations from Obligation Forests

The old solver maintains pending obligations in obligation forests, and PR 134348 demonstrated that these can be extracted for error reporting. A compatibility approach could standardize this extraction:

```rust
// In rustc_infer/src/traits/engine.rs or similar
pub enum SolverError<'tcx> {
    OldSolver(OldSolverError<'tcx>),
    NewSolver(NextSolverError<'tcx>),
}

pub struct OldSolverError<'tcx> {
    pub code: FulfillmentErrorCode<'tcx>,
    // Optional: pending obligations if available
    pub pending_obligations: Option<Vec<PredicateObligation<'tcx>>>,
}
```

The old solver's error collection code in fulfill.rs would populate this:

```rust
impl FromSolverError<'tcx, OldSolverError<'tcx>> for FulfillmentError<'tcx> {
    fn from_solver_error(
        infcx: &InferCtxt<'tcx>,
        error: OldSolverError<'tcx>
    ) -> Self {
        let root_obligation = extract_root_obligation(&error);
        let obligation = select_best_obligation(&error);
        FulfillmentError {
            obligation,
            code: error.code,
            root_obligation,
            // Store pending obligations in an associated field if needed
        }
    }
}
```

The extraction of pending obligations from the forest can happen during error collection:

```rust
fn collect_errors_with_pending(
    &self,
    forest: &ObligationForest<PendingPredicateObligation<'tcx>>,
) -> Vec<OldSolverError<'tcx>> {
    forest.to_errors(|obligation| {
        // Collect obligations still pending in the same tree
        let pending = forest.get_pending_in_tree(obligation.tree_id());
        
        OldSolverError {
            code: FulfillmentErrorCode::Select(SelectionError::Unimplemented),
            pending_obligations: Some(pending),
        }
    })
}
```

This extraction is opt-in: if the old solver cannot determine pending obligations for some reason, it sets the field to None, and error reporting falls back to traditional behavior.

### Adapting PR 134348 Approach for Completeness

PR 134348's core idea of reporting pending obligations can be enhanced to provide more complete information. The original PR simply listed all pending predicates, but we can improve this by:

1. Filtering pending obligations to focus on those from impl where clauses
2. Grouping related pending obligations under their common parent
3. Distinguishing root causes from transitive failures

The enhanced approach would modify `report_similar_impl_candidates`:

```rust
fn report_similar_impl_candidates_enhanced<'tcx>(
    &self,
    impl_def_id: DefId,
    trait_predicate: ty::PolyTraitPredicate<'tcx>,
    pending_obligations: &[PredicateObligation<'tcx>],
    err: &mut Diag<'_>,
) {
    // Group obligations by whether they come from where clauses
    let (where_clause_failures, other_failures): (Vec<_>, Vec<_>) =
        pending_obligations.iter().partition(|obl| {
            matches!(
                obl.cause.code(),
                ObligationCauseCode::WhereClause(..) | 
                ObligationCauseCode::ImplDerived(..)
            )
        });
    
    if !where_clause_failures.is_empty() {
        err.note("the following where clause constraints are not satisfied:");
        for failure in where_clause_failures {
            err.note(format!("  {}", failure.predicate));
        }
    }
    
    // Optionally report other failures if they're not redundant
    if !other_failures.is_empty() && where_clause_failures.is_empty() {
        err.note("the following constraints are not satisfied:");
        for failure in other_failures.take(3) {
            err.note(format!("  {}", failure.predicate));
        }
    }
}
```

This filtering provides more focused error messages by prioritizing where clause failures (which are usually what users need to address) while avoiding overwhelming output from other obligation types.

### Migration Path and Deprecation Strategy

The migration should follow a phased approach:

**Phase 1: Stabilize New Solver (6-12 months)**
- Focus all improvement effort on the new solver
- Old solver remains unchanged except for critical bug fixes
- New solver improvements can be tested with `-Znext-solver` flag
- Document known differences between solver error messages

**Phase 2: New Solver Beta (12-18 months)**
- New solver becomes default behind a feature flag
- Old solver remains available as fallback
- Error reporting improvements apply to both solvers where feasible
- Any breaking changes to error types are compatibility-wrapped

**Phase 3: Old Solver Deprecation (18-24 months)**
- New solver becomes unconditional default
- Old solver code paths are deprecated with warnings
- Error reporting code can assume new solver data structures
- Old solver error reporting becomes minimal stubs

**Phase 4: Old Solver Removal (24+ months)**
- Old solver implementation is removed from codebase
- Error reporting is simplified to only support new solver patterns
- All CGP error improvements are fully operational

This timeline is deliberately conservative, allowing ample time for the new solver to prove production-ready and for ecosystem migration. The error reporting improvements should be designed to function fully in Phase 1 with the new solver and provide degraded but functional support through Phase 3 with the old solver.

### Testing Strategy for Both Solvers

Testing must validate that:
1. Error messages from the new solver are improved as designed
2. Error messages from the old solver don't regress from baseline
3. Both solvers produce equivalent results modulo error message differences
4. The error type conversions preserve necessary information

The testing approach uses compiler test suites with solver-specific expectations:

```rust
// tests/ui/error-messages/cgp-missing-field.rs
use cgp::prelude::*;

#[derive(HasField)]
struct Rectangle {
    width: f64,
    // Missing height field
}

impl AreaCalculator for RectangleArea
where
    Self: HasRectangleFields,
{
    fn area(&self) -> f64 {
        self.width() * self.height()
    }
}

fn main() {
    Rectangle { width: 5.0 }.area();
}
```

With separate expectation files:

```
// tests/ui/error-messages/cgp-missing-field.next-solver.stderr
error[E0277]: the field `height` is not defined on type `Rectangle`
  --> tests/ui/error-messages/cgp-missing-field.rs:10:1
   |
10 | impl AreaCalculator for RectangleArea
   | ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
   |
note: required by `HasRectangleFields`
note: required by a bound in `RectangleArea::area`

// tests/ui/error-messages/cgp-missing-field.old-solver.stderr  
error[E0277]: the trait bound `Rectangle: HasField<"height">` is not satisfied
  --> tests/ui/error-messages/cgp-missing-field.rs:10:1
   |
10 | impl AreaCalculator for RectangleArea
   | ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
   |
   = help: the trait `HasField<"height">` is not implemented for `Rectangle`
```

The test infrastructure would automatically run tests with both solvers and verify the appropriate stderr file matches. This allows the new solver to have improved messages while ensuring the old solver's messages remain stable.

Integration tests should verify end-to-end behavior:

```rust
#[test]
fn test_cgp_error_contains_root_cause() {
    let output = compile_fail_with_new_solver("cgp-missing-field.rs");
    assert!(output.contains("field `height` is not defined"));
    assert!(output.contains("required by `HasRectangleFields`"));
}

#[test]
fn test_old_solver_compatibility() {
    let output = compile_fail_with_old_solver("cgp-missing-field.rs");
    // Verify old solver still produces valid error, even if less detailed
    assert!(output.contains("E0277"));
    assert!(output.contains("not satisfied"));
}
```

These integration tests validate the quality of error messages rather than just their exact text, allowing flexibility in message formatting while ensuring the essential information is present.

---

## Chapter 8: Detailed Implementation Roadmap

### Chapter Outline

This final chapter provides a phase-by-phase implementation roadmap with specific milestones, deliverables, testing requirements, and success criteria for each phase. The roadmap is designed to allow incremental progress with frequent validation points, minimizing risk while delivering value early.

### Phase 1: Proof of Concept for Leaf Constraint Collection

**Objective**: Demonstrate that comprehensive leaf constraint information can be collected from proof trees and used in error messages.

**Duration**: 2-3 weeks

**Deliverables**:
1. Modified `BestObligation` visitor that collects all leaf constraints
2. `LeafConstraint` type definition and associated APIs
3. Proof-of-concept error message formatter that displays collected constraints
4. Unit tests validating constraint collection

**Implementation Steps**:

Step 1: Define the `LeafConstraint` structure in derive_errors.rs:

```rust
pub struct LeafConstraint<'tcx> {
    pub predicate: ty::Predicate<'tcx>,
    pub span: Span,
    pub cause_chain: Vec<ObligationCauseCode<'tcx>>,
    pub source: GoalSource,
}
```

Step 2: Augment `BestObligation` to maintain a collection:

```rust
struct BestObligation<'tcx> {
    obligation: PredicateObligation<'tcx>,
    consider_ambiguities: bool,
    leaf_constraints: Vec<LeafConstraint<'tcx>>,
}
```

Step 3: Modify the visitor's `visit_goal` method to collect constraints when encountering leaves:

```rust
fn visit_goal(&mut self, goal: &inspect::InspectGoal<'_, 'tcx>) -> Self::Result {
    let candidates = self.non_trivial_candidates(goal);
    let candidate = match candidates.as_slice() {
        [candidate] => candidate,
        [] => {
            self.collect_leaf_constraint(goal);
            return self.detect_error_from_empty_candidates(goal);
        }
        _ => return ControlFlow::Break(self.obligation.clone()),
    };
    // ... rest remains unchanged ...
}
```

Step 4: Implement `collect_leaf_constraint`:

```rust
fn collect_leaf_constraint(&mut self, goal: &inspect::InspectGoal<'_, 'tcx>) {
    let predicate = goal.goal().predicate;
    let cause_chain = self.extract_cause_chain();
    
    self.leaf_constraints.push(LeafConstraint {
        predicate: predicate.upcast(goal.infcx().tcx),
        span: self.obligation.cause.span,
        cause_chain,
        source: GoalSource::Misc,
    });
}
```

Step 5: Modify `find_best_leaf_obligation` to return leaf constraints alongside the primary obligation:

```rust
fn find_best_leaf_obligation<'tcx>(
    infcx: &InferCtxt<'tcx>,
    obligation: &PredicateObligation<'tcx>,
    consider_ambiguities: bool,
) -> (PredicateObligation<'tcx>, Vec<LeafConstraint<'tcx>>) {
    // ... existing resolution and visitation code ...
    let mut visitor = BestObligation {
        obligation: obligation.clone(),
        consider_ambiguities,
        leaf_constraints: Vec::new(),
    };
    
    let obligation = /* ... existing visitation logic ... */;
    let constraints = visitor.leaf_constraints;
    
    (obligation, constraints)
}
```

**Testing**:
- Create test cases with known deep dependency chains (CGP-style examples)
- Verify that leaf constraints collection captures all expected predicates
- Ensure the primary obligation selection logic remains unchanged
- Validate that cause chain extraction produces accurate chains

**Success Criteria**:
- All leaf constraints from a proof tree are captured without duplicates
- The primary obligation selection matches existing behavior
- Performance overhead is measureable but negligible (< 5% regression on trait-heavy compilation)

### Phase 2: Dependency Graph Construction

**Objective**: Build explicit dependency graphs from proof tree information and use them to identify root causes.

**Duration**: 3-4 weeks

**Deliverables**:
1. `DependencyGraph` type and construction algorithm
2. Root cause identification algorithm
3. Integration with `fulfillment_error_for_no_solution`
4. Unit tests for dependency graph properties

**Implementation Steps**:

Step 1: Define dependency graph types in a new module `rustc_trait_selection/src/solve/dependency_graph.rs`:

```rust
pub struct DependencyGraph<'tcx> {
    nodes: FxIndexMap<ty::Predicate<'tcx>, NodeInfo<'tcx>>,
    edges: Vec<DependencyEdge<'tcx>>,
}

pub struct NodeInfo<'tcx> {
    predicate: ty::Predicate<'tcx>,
    failed: bool,
    is_leaf: bool,
    span: Span,
    source_impl: Option<DefId>,
}

pub struct DependencyEdge<'tcx> {
    from: ty::Predicate<'tcx>,
    to: ty::Predicate<'tcx>,
    kind: DependencyKind,
}

pub enum DependencyKind {
    ImplWhereClause { impl_def_id: DefId, predicate_index: usize },
    SuperTrait,
    WellFormedness,
}
```

Step 2: Implement graph construction from leaf constraints:

```rust
impl<'tcx> DependencyGraph<'tcx> {
    pub fn from_leaf_constraints(
        root: &PredicateObligation<'tcx>,
        constraints: &[LeafConstraint<'tcx>],
    ) -> Self {
        let mut graph = DependencyGraph {
            nodes: FxIndexMap::default(),
            edges: Vec::new(),
        };
        
        graph.add_node(root.predicate, NodeInfo {
            predicate: root.predicate,
            failed: true,
            is_leaf: false,
            span: root.cause.span,
            source_impl: None,
        });
        
        for constraint in constraints {
            graph.add_leaf_constraint(constraint);
        }
        
        graph
    }
    
    fn add_leaf_constraint(&mut self, constraint: &LeafConstraint<'tcx>) {
        self.add_node(constraint.predicate, NodeInfo {
            predicate: constraint.predicate,
            failed: true,
            is_leaf: true,
            span: constraint.span,
            source_impl: None,
        });
        
        self.extract_edges_from_cause_chain(constraint);
    }
    
    fn extract_edges_from_cause_chain(&mut self, constraint: &LeafConstraint<'tcx>) {
        for (i, code) in constraint.cause_chain.iter().enumerate() {
            match code {
                ObligationCauseCode::ImplDerived(inner) => {
                    let from_pred = inner.derived.parent_trait_pred.upcast();
                    self.ensure_node(from_pred);
                    self.edges.push(DependencyEdge {
                        from: from_pred,
                        to: if i == 0 { constraint.predicate } else { /* previous in chain */ },
                        kind: DependencyKind::ImplWhereClause {
                            impl_def_id: inner.impl_or_alias_def_id,
                            predicate_index: inner.impl_def_predicate_index.unwrap_or(0),
                        },
                    });
                }
                _ => {}
            }
        }
    }
}
```

Step 3: Implement root cause identification:

```rust
impl<'tcx> DependencyGraph<'tcx> {
    pub fn identify_root_causes(&self, tcx: TyCtxt<'tcx>) -> Vec<RootCause<'tcx>> {
        let mut causes = Vec::new();
        
        for (pred, info) in &self.nodes {
            if info.is_leaf {
                let cause = self.classify_root_cause(*pred, tcx);
                causes.push(cause);
            }
        }
        
        deduplicate_causes(&mut causes);
        causes
    }
    
    fn classify_root_cause(&self, pred: ty::Predicate<'tcx>, tcx: TyCtxt<'tcx>) -> RootCause<'tcx> {
        // Classification logic as designed in Chapter 6
    }
}
```

Step 4: Integrate with error derivation:

```rust
pub(super) fn fulfillment_error_for_no_solution<'tcx>(
    infcx: &InferCtxt<'tcx>,
    root_obligation: PredicateObligation<'tcx>,
) -> FulfillmentError<'tcx> {
    let (obligation, leaf_constraints) = find_best_leaf_obligation(infcx, &root_obligation, false);
    
    // Build dependency graph
    let graph = DependencyGraph::from_leaf_constraints(&root_obligation, &leaf_constraints);
    
    // Identify root causes
    let root_causes = graph.identify_root_causes(infcx.tcx);
    
    // Store in fulfillment error for error reporting to access
    let code = construct_error_code_with_root_causes(obligation, root_causes);
    
    FulfillmentError { obligation, code, root_obligation }
}
```

**Testing**:
- Graph construction tests with known dependency structures
- Root cause identification tests with various predicate types
- End-to-end tests using simple CGP examples
- Performance tests ensuring graph construction doesn't significantly slow compilation

**Success Criteria**:
- Dependency graphs accurately represent proof tree structure
- Root cause identification correctly distinguishes真root causes from transitive failures
- Integration with error derivation is seamless
- Performance impact remains acceptable (< 10% regression on trait-heavy code)

### Phase 3: Traceable Attribute Integration

**Objective**: Implement the `#[diagnostic::traceable]` attribute and integrate it with error filtering.

**Duration**: 2-3 weeks

**Deliverables**:
1. Attribute parsing and storage in trait definitions
2. Error reporting checks for traceable attribute
3. Integration with dependency graph filtering
4. Documentation and examples

**Implementation Steps**:

Step 1: Define the attribute in `rustc_feature/src/builtin_attrs.rs`:

```rust
rustc_attr!(diagnostic: traceable, Whitelisted, template(Word)),
```

Step 2: Add storage to trait definitions in `rustc_middle/src/ty/trait_def.rs`:

```rust
pub struct TraitDef {
    // ... existing fields ...
    pub has_traceable_bounds: bool,
}
```

Step 3: Parse the attribute during trait definition checking:

```rust
fn check_trait_def(tcx: TyCtxt<'_>, trait_def_id: LocalDefId) {
    let attrs = tcx.get_attrs(trait_def_id.to_def_id());
    let has_traceable = attrs.iter().any(|attr| attr.has_name(sym::diagnostic));
    
    // Store in trait definition
    tcx.trait_def(trait_def_id).has_traceable_bounds = has_traceable;
}
```

Step 4: Modify error filtering to check attribute:

```rust
fn should_report_constraint<'tcx>(
    predicate: ty::Predicate<'tcx>,
    graph: &DependencyGraph<'tcx>,
    tcx: TyCtxt<'tcx>,
) -> bool {
    if let Some(trait_pred) = predicate.as_trait_clause() {
        let trait_def = tcx.trait_def(trait_pred.def_id());
        if trait_def.has_traceable_bounds {
            return true;
        }
    }
    
    // Apply normal filtering for non-traceable predicates
    apply_standard_filtering(predicate, graph)
}
```

Step 5: Update CGP macros to apply attribute:

```rust
// In cgp crate macros
#[proc_macro_attribute]
pub fn cgp_provider(attr: TokenStream, item: TokenStream) -> TokenStream {
    let mut output = quote! {
        #[diagnostic::traceable]
        #item
    };
    // ... rest of macro implementation ...
}
```

**Testing**:
- Attribute parsing tests
- Tests verifying traceable bounds are never filtered
- Tests confirming non-traceable bounds use normal filtering
- CGP integration tests with and without attribute

**Success Criteria**:
- Attribute is recognized and stored correctly
- Error filtering respects the attribute
- CGP error messages include all provider constraints
- Non-CGP code is not affected by the attribute

### Phase 4: Error Message Formatting Improvements

**Objective**: Implement specialized formatting for CGP patterns and enhanced error messages using dependency graph information.

**Duration**: 3-4 weeks

**Deliverables**:
1. CGP pattern recognition in error formatting
2. Type-level construct formatting (Symbol types, etc.)
3. Enhanced error message templates using root causes
4. Updated error reporting documentation

**Implementation Steps**:

Step 1: Implement pattern recognition in fulfillment_errors.rs:

```rust
fn recognize_cgp_pattern<'tcx>(
    trait_ref: ty::TraitRef<'tcx>,
    tcx: TyCtxt<'tcx>,
) -> Option<CgpPattern<'tcx>> {
    // Check known CGP trait names
    let trait_name = tcx.item_name(trait_ref.def_id);
    
    if trait_name.as_str() == "IsProviderFor" {
        return Some(CgpPattern::IsProviderFor {
            component: extract_arg(trait_ref, 0),
            context: trait_ref.self_ty(),
            provider: extract_arg(trait_ref, 1),
        });
    }
    
    None
}

enum CgpPattern<'tcx> {
    IsProviderFor {
        component: GenericArg<'tcx>,
        context: Ty<'tcx>,
        provider: Ty<'tcx>,
    },
    DelegateComponent {
        component: GenericArg<'tcx>,
        context: Ty<'tcx>,
    },
}
```

Step 2: Implement specialized formatting:

```rust
fn format_cgp_error<'tcx>(
    pattern: CgpPattern<'tcx>,
    root_causes: &[RootCause<'tcx>],
    err: &mut Diag<'_>,
) {
    match pattern {
        CgpPattern::IsProviderFor { component, context, provider } => {
            err.message(format!(
                "provider `{}` cannot satisfy component `{}` for `{}`",
                provider, component, context
            ));
            
            for cause in root_causes {
                match cause {
                    RootCause::MissingField { struct_ty, field_name } => {
                        err.note(format!(
                            "the field `{}` is missing from `{}`",
                            field_name, struct_ty
                        ));
                    }
                    RootCause::UnsatisfiedBound { bound, .. } => {
                        err.note(format!(
                            "the constraint `{}` is not satisfied",
                            bound
                        ));
                    }
                    _ => {}
                }
            }
        }
        _ => {}
    }
}
```

Step 3: Integrate with main error reporting:

```rust
pub fn report_selection_error(
    &self,
    obligation: PredicateObligation<'tcx>,
    root_obligation: &PredicateObligation<'tcx>,
    error: &SelectionError<'tcx>,
) -> ErrorGuaranteed {
    // ... existing code ...
    
    // Check for CGP patterns
    if let Some(pattern) = recognize_cgp_pattern(trait_ref, self.tcx) {
        if let Some(root_causes) = extract_root_causes_from_error(&error) {
            return format_cgp_error(pattern, &root_causes, &mut err).emit();
        }
    }
    
    // ... fallback to standard formatting ...
}
```

**Testing**:
- Pattern recognition tests for all CGP traits
- Formatting tests comparing output with expected messages
- End-to-end tests with actual CGP code
- Tests verifying non-CGP code isn't affected

**Success Criteria**:
- CGP errors use component/provider terminology
- Root causes appear prominently in error messages
- Type-level constructs are formatted readably
- Non-CGP error messages remain unchanged

### Phase 5: Old Solver Compatibility Layer

**Objective**: Ensure old solver error reporting remains functional during transition period.

**Duration**: 2-3 weeks

**Deliverables**:
1. Compatibility error type conversions
2. Pending obligation extraction from obligation forests
3. Feature flag for testing both solvers
4. Comparative test suite

**Implementation Steps**:

Step 1: Implement pending obligation extraction in old fulfillment context:

```rust
impl<'tcx> FulfillmentContext<'tcx> {
    pub fn extract_pending_obligations(&self) -> Vec<PredicateObligation<'tcx>> {
        self.predicates.nodes.iter()
            .filter(|node| node.state.is_pending())
            .map(|node| node.obligation.obligation.clone())
            .collect()
    }
}
```

Step 2: Create compatibility wrapper:

```rust
pub struct OldSolverError<'tcx> {
    pub code: FulfillmentErrorCode<'tcx>,
    pub pending_obligations: Option<Vec<PredicateObligation<'tcx>>>,
}

impl<'tcx> FromSolverError<'tcx, OldSolverError<'tcx>> for FulfillmentError<'tcx> {
    fn from_solver_error(infcx: &InferCtxt<'tcx>, error: OldSolverError<'tcx>) -> Self {
        // Convert old solver error to standard FulfillmentError
        // Extract root obligation from pending obligations if available
    }
}
```

Step 3: Modify error reporting to handle both error types gracefully:

```rust
fn report_fulfillment_errors<'tcx>(
    &self,
    errors: &[FulfillmentError<'tcx>],
) {
    for error in errors {        // Check if we have enhanced information available
        if let Some(root_causes) = try_extract_root_causes(error) {
            // Use enhanced reporting with root causes
            self.report_with_root_causes(error, &root_causes);
        } else {
            // Fall back to traditional reporting
            self.report_traditional(error);
        }
    }
}
```

**Testing**:
- Comparative tests running same code with both solvers
- Verification that old solver messages don't regress
- Feature flag tests ensuring correct solver is used
- Performance comparison between solvers

**Success Criteria**:
- Old solver error messages remain stable or improve slightly
- New solver error messages show full improvements
- Both solvers can be selected and produce valid errors
- Performance difference is acceptable

### Phase 6: Testing, Documentation, and Stabilization

**Objective**: Comprehensive testing, documentation, and preparation for stabilization.

**Duration**: 4-6 weeks

**Deliverables**:
1. Comprehensive test suite covering all error scenarios
2. Documentation for library authors using `#[diagnostic::traceable]`
3. User-facing documentation on interpreting improved error messages
4. Performance benchmarks and optimization if needed
5. Stabilization report for compiler team review

**Implementation Steps**:

Step 1: Develop comprehensive test suite:

```rust
// tests/ui/error-messages/cgp/*.rs
// - basic single-level delegation errors
// - multi-level delegation chain errors
// - multiple independent root causes
// - mixed CGP and non-CGP errors
// - edge cases (empty implementations, conflicting bounds, etc.)
```

Step 2: Write library author guide:

```markdown
# Using #[diagnostic::traceable] for Better Error Messages

The `#[diagnostic::traceable]` attribute ensures that constraints
from your trait's where clauses are always reported explicitly
in error messages, even in deep dependency chains.

## When to Use

- Provider traits in CGP frameworks
- Foundational traits with critical where clause requirements
- Traits used in complex generic contexts

## Example

\```rust
#[diagnostic::traceable]
pub trait IsProviderFor<Component, Context> {
    // ...
}
\```

## Impact

With this attribute, errors will always mention specific unsatisfied
constraints from your trait, helping users understand what they need
to implement.
```

Step 3: Create user guide for interpreting improved errors:

```markdown
# Understanding CGP Error Messages

When Context-Generic Programming code has errors, the compiler
now provides detailed information about root causes.

## Anatomy of a CGP Error

```
error: provider `RectangleArea` cannot satisfy component
       `AreaCalculatorComponent` for `Rectangle`
  --> src/main.rs:45:1
   |
45 | delegate_components! { ... }
   | ^^^^^^^^^^^^^^^^^^^^^^^^^^^^
   |
note: the field `height` is missing from `Rectangle`
  --> src/main.rs:10:1
   |
10 | struct Rectangle { width: f64 }
   | ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
   |
 = help: add `height: f64` field to `Rectangle`
```

The error shows:
1. The high-level component that failed
2. The specific root cause (missing field)
3. Actionable fix suggestion
```

Step 4: Run performance benchmarks:

```rust
// benchmarks/error-reporting.rs
#[bench]
fn bench_deep_cgp_error(b: &mut Bencher) {
    let input = load_cgp_test_case("deep-delegation-chain");
    b.iter(|| compile_with_errors(input));
}

#[bench]
fn bench_non_cgp_error(b: &mut Bencher) {
    let input = load_test_case("standard-trait-error");
    b.iter(|| compile_with_errors(input));
}
```

Step 5: Prepare stabilization report:

```markdown
# Stabilization Report: Enhanced Error Reporting for Deep Trait Dependencies

## Summary
This feature improves error messages when trait obligations fail
deep within dependency chains, particularly benefiting libraries
like CGP that use extensive blanket implementations.

## Implementation Quality
- New solver implementation: Complete and tested
- Old solver compatibility: Provided with graceful degradation
- Performance impact: < 5% on error cases, negligible on success cases
- Test coverage: 95%+ of error scenarios

## User Impact
- Positive: Users of CGP and similar libraries see actionable errors
- Neutral: Most Rust code sees no change in error messages
- No breaking changes to existing functionality

## Recommendation
Ready for stabilization in Rust 1.XX
```

**Testing**:
- Full test suite execution on multiple platforms
- Integration testing with real CGP codebases
- Performance regression testing
- User acceptance testing with CGP community

**Success Criteria**:
- All tests pass on all platforms
- Performance Within acceptable bounds (< 5% regression)
- Documentation complete and reviewed
- Positive feedback from CGP users
- Compiler team approval for stabilization

---

## Conclusion

This comprehensive analysis has examined the Rust compiler's trait solving and error reporting architecture from multiple angles, providing both deep understanding of current mechanisms and specific actionable proposals for improvement. The next-generation trait solver provides a solid foundation for implementing enhanced error reporting through its proof tree visitor pattern and derived cause construction. The implementation can proceed incrementally through six phases, delivering value at each stage while maintaining compatibility and minimizing risk.

The key insight is that the information needed for better error messages already exists within the compiler's data structures; the challenge is extracting, organizing, and presenting that information effectively. By combining proof tree traversal for comprehensive constraint collection, dependency graph analysis for root cause identification, the `#[diagnostic::traceable]` attribute for library author control, and specialized formatting for CGP patterns, we can dramatically improve error message quality for complex generic code without sacrificing performance or breaking existing functionality.

The proposed improvements focus primarily on the new solver, recognizing that the old solver is being phased out and that investment in old solver improvements has diminishing returns. A minimal compatibility layer ensures the old solver continues functioning adequately during the transition while the focus remains on delivering full functionality in the new solver that represents Rust's future.

Success will be measured not just by technical implementation completeness but by practical impact: CGP users should be able to diagnose and fix errors quickly based on compiler messages, without extensive manual investigation of trait implementations and where clauses. The broader Rust community should see no negative impact, with potential improvements for any code using deep blanket implementation chains. This balanced approach respects the compiler team's concerns about error message quality across diverse codebases while addressing the real needs of advanced trait system users.
