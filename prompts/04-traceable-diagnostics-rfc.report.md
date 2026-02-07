# RFC: Introducing `#[diagnostic::traceable]` for Enhanced Trait Error Diagnostics

## Summary

This RFC proposes adding a new diagnostic attribute `#[diagnostic::traceable]` that instructs the Rust compiler to always report unsatisfied trait bounds in error messages, even when they appear deep within dependency chains. Currently, the compiler's error reporting heuristics filter out what it considers "transitive" failures to keep error messages concise, but these heuristics can inadvertently suppress root causes when dealing with deep blanket trait implementation chains. The `#[diagnostic::traceable]` attribute provides library authors with a mechanism to mark specific trait bounds as semantically significant, ensuring that failures to satisfy these bounds are always visible to users regardless of how deeply nested they appear in the trait resolution hierarchy.

The attribute solves a fundamental tension in the compiler's error reporting system. For traditional Rust code with shallow trait bound dependencies, aggressive filtering produces clear and actionable error messages. However, certain advanced patterns including Context-Generic Programming deliberately create deep delegation chains where the distinction between "transitive" and "root cause" failures does not align with the compiler's heuristics. Without intervention, users of these patterns receive error messages that report symptoms while hiding the underlying cause, forcing them to manually trace through implementation details to diagnose problems.

The proposed attribute has minimal impact on existing code since it is purely additive and opt-in. Library authors who maintain code with deep trait bound dependencies can annotate their blanket implementations to ensure that critical bounds remain visible in error messages. Users of these libraries benefit from improved diagnostics without any changes to their own code. The implementation leverages existing obligation cause tracking mechanisms in both the current trait solver and the next-generation solver, requiring modifications primarily to error filtering and reporting logic rather than fundamental changes to the trait resolution engine.

## Table of Contents

### Chapter 1: Motivation and Problem Statement
1.1 The Challenge of Error Reporting in Generic Code
1.2 How Blanket Implementations Create Deep Dependency Chains
1.3 The Compiler's Current Filtering Heuristics and Their Limitations
1.4 Real-World Impact on Library Users
1.5 Why Existing Diagnostic Tools Are Insufficient

### Chapter 2: Design Philosophy and Core Semantics
2.1 The Principle of Semantic Significance in Trait Bounds
2.2 Attribute Placement and Scope
2.3 Interaction with Trait Resolution and Obligation Processing
2.4 Relationship to Existing Diagnostic Attributes
2.5 Forward Compatibility and Evolution Path

### Chapter 3: Detailed Specification
3.1 Syntax and Grammar
3.2 Semantic Rules for Trait Implementations
3.3 Semantic Rules for Trait Definitions
3.4 Semantic Rules for Associated Types and Constants
3.5 Error Conditions and Validation

### Chapter 4: Simple Examples with Vanilla Rust
4.1 Basic Blanket Implementation Without Traceable
4.2 Applying Traceable to Expose Hidden Constraints
4.3 Multiple Layers of Delegation
4.4 Interaction with Multiple Trait Bounds
4.5 Comparison of Error Messages Before and After

### Chapter 5: Application to Context-Generic Programming
5.1 Brief Overview of CGP Patterns
5.2 Provider-Consumer Delegation Chains
5.3 Eliminating the Need for Workaround Traits
5.4 Higher-Order Providers and Nested Dependencies
5.5 Complete Error Message Transformation Examples

### Chapter 6: Application to Other Patterns
6.1 Type-Level Computation and Associated Types
6.2 Builder Patterns with Compile-Time Validation
6.3 Effect Systems and Capability-Based Designs
6.4 Phantom Type Constraints and Zero-Cost Abstractions

### Chapter 7: Implementation Strategy
7.1 Obligation Cause Enhancement
7.2 Error Filtering Modifications
7.3 Proof Tree Analysis in the New Solver
7.4 Obligation Forest Handling in the Old Solver
7.5 Error Message Generation and Formatting

### Chapter 8: Impact Analysis and Considerations
8.1 Performance Implications
8.2 Error Message Verbosity Trade-offs
8.3 Interaction with Future Language Features
8.4 Migration Path for Existing Code
8.5 Documentation and User Guidance

### Chapter 9: Alternatives Considered
9.1 Compiler Flags for Diagnostic Verbosity
9.2 Heuristic Improvements Without Attribute
9.3 Trait-Level Annotations vs Implementation-Level Annotations
9.4 Automatic Detection of Deep Chains
9.5 Why the Proposed Design is Superior

### Chapter 10: Unresolved Questions and Future Work
10.1 Interaction with Specialization
10.2 Negative Trait Bounds and Exclusion Constraints
10.3 Diagnostic Annotations for Associated Types
10.4 Integration with IDE Quick Fixes
10.5 Telemetry and Usage Patterns

---

## Chapter 1: Motivation and Problem Statement

### Chapter Outline

This chapter establishes the fundamental problem that `#[diagnostic::traceable]` addresses by examining how generic code creates error reporting challenges in Rust. We begin by exploring the general difficulty of providing useful error messages when trait resolution fails in generic contexts, then examine how blanket implementations specifically create dependency chains that confound current error reporting heuristics. We analyze the compiler's existing filtering strategies and identify where they break down, demonstrate the real-world impact on users through concrete examples, and explain why existing diagnostic tools cannot adequately address this problem. The chapter builds a comprehensive case for why a new mechanism is necessary.

### 1.1 The Challenge of Error Reporting in Generic Code

Rust's trait system enables powerful abstractions through generic programming, but this power comes with diagnostic complexity. When the compiler checks that a concrete type satisfies a trait bound, it must verify not just that an implementation exists, but that all constraints required by that implementation are also satisfied. Each constraint may itself require further verification, creating a potentially deep tree of obligations that must all be fulfilled for the original trait bound to hold.

Consider a simple generic function that requires its type parameter to implement a trait. If the type does not implement the trait, the compiler reports this directly and the user immediately understands what is missing. However, if the type would implement the trait through a blanket implementation that has unsatisfied constraints in its where clause, the diagnostic challenge becomes more complex. Should the compiler report that the type fails to implement the trait, or should it report that the underlying constraints required by the blanket implementation are not satisfied?

The answer depends on the user's mental model and what information enables them to fix the problem most efficiently. If the blanket implementation is considered an implementation detail and the trait itself is the public contract, reporting the trait failure may be appropriate. If the blanket implementation's constraints represent semantically meaningful requirements that users are expected to understand and satisfy, reporting the constraint failures provides more actionable information. The compiler must make this determination automatically without explicit guidance from library authors.

Current Rust compiler heuristics assume that shallow constraint chains indicate public contracts that should be reported directly, while deep chains indicate implementation details that should be abstracted away. For many common Rust patterns, this assumption holds reasonably well. Standard library traits like Iterator, where most implementations are explicit rather than derived through blanket implementations, generate straightforward error messages. However, libraries that deliberately use blanket implementations as their primary abstraction mechanism violate these assumptions, causing the heuristics to suppress precisely the information users need.

The fundamental challenge is that the compiler cannot automatically distinguish between implementation details that should be hidden and semantic requirements that must be visible. A blanket implementation's constraints might represent internal plumbing that users should not need to think about, or they might represent essential prerequisites that define when the blanket implementation applies. Without additional information from library authors, the compiler must guess, and when it guesses wrong, error messages become misleading or incomplete.

### 1.2 How Blanket Implementations Create Deep Dependency Chains

Blanket implementations are a powerful feature that allows implementing a trait for any type that satisfies certain constraints. The canonical example is implementing a trait for all types that implement another trait, enabling automatic trait derivation. When blanket implementations are chained, where one blanket implementation's constraints reference traits that are themselves implemented through other blanket implementations, the result is a delegation chain that can extend multiple levels deep.

To make this concrete, consider a trait hierarchy for serialization. A trait Serialize might have a blanket implementation for all types implementing ToJson, and ToJson might have a blanket implementation for all types implementing Display and Debug. If a user attempts to serialize a type that implements Display but not Debug, the compiler must decide whether to report the failure as the type not implementing Serialize, the type not implementing ToJson, or the type not implementing Debug. Each of these is technically accurate but provides different amounts of actionable information.

The deeper the chain, the more complex this decision becomes. If the chain extends five or six levels deep, the compiler's error message might report a failure at level three, providing neither the high-level context of what the user was trying to do nor the low-level detail of what specific capability is actually missing. The middle-level failure is technically where the chain breaks, but it is neither the root cause nor the symptom that the user directly observes.

Blanket implementations become particularly problematic when they encode conditional logic through trait bounds. A library might provide multiple blanket implementations for the same trait with different constraints, effectively implementing a form of compile-time pattern matching. When none of the patterns match, the compiler must report which constraints failed for which implementation candidates, but its heuristics may suppress some of this information if it appears to be transitive.

The issue is compounded when blanket implementations reference associated types or other generic parameters. The compiler must not only track which trait bounds failed but also which type parameters those bounds apply to and how those type parameters relate to the user's concrete types. Error messages that describe failures in terms of intermediate generic parameters rather than the concrete types the user wrote become difficult to interpret even when the reported information is technically correct.

### 1.3 The Compiler's Current Filtering Heuristics and Their Limitations

The Rust compiler applies multiple layers of filtering to trait resolution errors before presenting them to users. The first layer identifies which failed obligations are independent versus which are consequences of other failures. When obligation A could not be satisfied because obligation B failed and obligation B is a prerequisite for A, reporting both would be redundant. The compiler attempts to report only obligation B as the root cause while suppressing A as a transitive failure.

This filtering is implemented by examining the obligation cause chain that the trait solver maintains. Each obligation records why it was generated, with possibilities including that it comes from a where clause predicate in an implementation being applied, from a well-formedness check, from normalizing an associated type, or from other sources. When reporting errors, the compiler walks these cause chains to identify leaf obligations that have no further dependencies.

The second filtering layer attempts to identify which obligations represent user-facing contracts versus implementation details. The heuristic used is primarily based on the depth of the cause chain and the source location of the obligations. Obligations that arise close to user code are prioritized over those that arise deep within library code. Obligations with short cause chains are prioritized over those with long chains. The assumption is that long chains through library code represent internal implementation details.

These heuristics work well for code that follows certain patterns. When trait implementations are mostly explicit and blanket implementations are used sparingly for well-known derivation patterns, the heuristics correctly identify that a failure in a blanket implementation's constraints is more informative than reporting the blanket trait itself as unimplemented. However, when blanket implementations are the primary mechanism for providing functionality, the heuristics break down.

The specific failure mode is that the compiler identifies intermediate obligations in a delegation chain as leaf obligations because they have no further nested obligations at that point in the proof tree. An obligation to implement trait T might fail not because trait T itself cannot be implemented, but because the user has not configured the necessary supporting traits that T's blanket implementation depends on. The compiler sees that T cannot be implemented and reports this, while suppressing information about the supporting traits that are actually missing.

The filtering logic is further complicated by the compiler's batched error processing. When multiple obligations fail, the compiler attempts to group related failures and report them coherently. However, the grouping heuristics may separate a root cause from its consequences if they appear in different parts of the obligation forest or proof tree. The result is that users may see multiple seemingly unrelated error messages that are actually different manifestations of the same underlying problem.

### 1.4 Real-World Impact on Library Users

The practical impact of these diagnostic limitations is that users attempting to use libraries with deep blanket implementation chains face a frustrating debugging experience. When they fail to satisfy some constraint required deep within the delegation chain, they receive error messages stating that some intermediate trait is not implemented, without any indication of why it is not implemented or what they need to do to make it implemented.

The typical workflow becomes a manual depth-first search through library documentation and source code. The user sees that trait A is not implemented, examines the library to find blanket implementations for trait A, reads the where clauses of those implementations to identify trait B as a requirement, checks whether their type implements trait B, and if not, repeats the process to understand what is required for trait B. For a five-level delegation chain, this process must be repeated five times, and at each step the user must parse potentially complex generic constraints.

This debugging process is particularly painful because the information the user needs is available to the compiler but is deliberately being filtered out of the error message. The user is not discovering new information that the compiler lacks; they are reconstructing information that the compiler already computed during trait resolution but chose not to present. From the user's perspective, the compiler appears to be withholding critical diagnostic information for no apparent reason.

The impact is especially severe for users who are new to a library or to Rust's trait system in general. Experienced users develop intuitions about common patterns and can more quickly navigate through blanket implementation chains to identify root causes. Novice users lack these intuitions and may become completely stuck, unable to make forward progress without external help. The error messages provide no educational value and do not help users build mental models of how the library's abstractions work.

Library authors face a difficult choice. They can design their libraries with shallow trait hierarchies to ensure better error messages, sacrificing abstraction and code reuse. They can implement custom derive macros that generate explicit implementations rather than using blanket implementations, trading off flexibility for diagnostic clarity. Or they can accept that their libraries will be difficult to debug and invest in extensive documentation and examples to help users work around the poor error messages. All of these options represent suboptimal tradeoffs imposed by compiler limitations.

### 1.5 Why Existing Diagnostic Tools Are Insufficient

The Rust compiler provides several existing mechanisms for improving error messages, but none adequately address the deep delegation chain problem. The `#[rustc_on_unimplemented]` attribute allows trait authors to customize the error message when a type fails to implement their trait. However, this attribute can only customize the message when the trait itself is determined to be unimplemented; it cannot force the compiler to report underlying constraint failures that the compiler has classified as transitive.

When a blanket implementation's constraint fails, the `#[rustc_on_unimplemented]` message on the target trait may never be triggered because the compiler reports the constraint failure instead. Even if the message is triggered, it cannot access information about which specific constraints failed or why they failed. The attribute can provide general guidance about what kinds of types are expected to implement the trait, but it cannot diagnose the specific problem with the user's particular type.

The compiler's obligation forest and proof tree structures contain all the information necessary to generate complete diagnostic messages, but this information is filtered before reaching the error reporting layer. Users cannot opt into more verbose diagnostics short of modifying the compiler itself. There is no flag or configuration option that tells the compiler to report all failed obligations rather than applying heuristic filtering. The filtering is hardcoded into the error reporting logic and applies uniformly to all code.

Some library authors have attempted to work around these limitations by introducing marker traits whose sole purpose is to force certain obligations to be reported. These marker traits have explicit implementations rather than blanket implementations, ensuring that when they are unsatisfied, the compiler treats this as a leaf failure. However, this approach pollutes the library's public API with traits that exist only for diagnostic purposes, increases implementation burden, and does not scale well to complex constraint patterns.

The fundamental limitation is that existing mechanisms operate at the wrong level of abstraction. They can customize messages for specific failures or change how individual trait implementations are reported, but they cannot modify the compiler's filtering logic to preserve information that would otherwise be suppressed. What is needed is a mechanism that communicates semantic intent from library authors to the compiler's error reporting system, specifically indicating which trait bounds represent meaningful semantic requirements that must always be visible in error messages.

---

## Chapter 2: Design Philosophy and Core Semantics

### Chapter Outline

This chapter articulates the design principles underlying `#[diagnostic::traceable]` and defines its core semantics. We begin by establishing the fundamental concept of semantic significance in trait bounds and how this relates to error reporting. We then specify where the attribute can be placed and what scope it affects, explain how it interacts with the trait resolution and obligation processing machinery, position it relative to existing diagnostic attributes, and discuss forward compatibility considerations for future language evolution. The goal is to provide a clear conceptual framework before diving into technical specification details.

### 2.1 The Principle of Semantic Significance in Trait Bounds

The core insight motivating `#[diagnostic::traceable]` is that not all trait bounds in a where clause serve the same semantic role. Some bounds express implementation details that users of an abstraction should not need to understand. Other bounds express fundamental requirements that define when the abstraction is applicable. The compiler cannot automatically distinguish these cases, but library authors possess this knowledge as part of their API design decisions.

A semantically significant trait bound is one that represents a meaningful requirement in the library's public contract. When such a bound is unsatisfied, this represents a genuine gap in capability that the user must address, not merely an intermediate failure in a longer derivation. Marking a bound as traceable communicates to the compiler that failures to satisfy this bound should always be reported clearly, even if the bound appears deep within a chain of implications.

Consider a generic function that operates on types implementing multiple traits. Some of these trait bounds might be fundamental to what the function does, expressing capabilities that the function directly relies on. Other bounds might be implementation artifacts, required only because the function's implementation uses certain helper traits internally. If the function's signature mentions a bound that is actually satisfied through a blanket implementation requiring additional traits, those additional traits might be implementation details or might be semantically essential parts of the function's contract.

The traceable attribute allows library authors to make this distinction explicit. When a bound is marked traceable, the author asserts that understanding why this bound might fail is important for users of the library. The compiler should prioritize reporting failures of traceable bounds over failures of unmarked bounds, and should ensure that traceable bound failures are not suppressed as transitive even if they appear deep in a dependency chain.

This design respects the principle that library authors are best positioned to understand their own abstractions and make API design decisions. Just as authors decide which types and traits to expose publicly versus keep private, they can decide which trait bound failures represent important diagnostic information versus noise. The compiler's role is to honor these annotations consistently and reliably, not to second-guess the author's judgment about what is semantically significant.

### 2.2 Attribute Placement and Scope

The `#[diagnostic::traceable]` attribute can be applied to trait bounds in where clauses of trait implementations. When applied, it marks that specific bound as semantically significant within the context of that implementation. The attribute scope is deliberately narrow, affecting only how failures of that bound are reported when checking whether the implementation applies, not changing the trait's semantics or the implementation's behavior.

The attribute is written directly before a trait bound in a where clause, following Rust's existing pattern for attributes on clause items. Multiple bounds in the same where clause can be independently marked traceable or left unmarked. The attribute applies only to the immediately following bound and does not affect other bounds even if they are connected by boolean operators.

The attribute cannot be applied to the entire implementation block, to the trait definition, or to individual methods. This scoping ensures that the attribute has well-defined semantics localized to specific trait bound checks. Applying it to an implementation block would be ambiguous about which bounds it refers to, while applying it to a trait definition would impose a global policy that might not be appropriate for all implementations of that trait.

When the compiler checks whether a particular implementation can satisfy an obligation, it examines the implementation's where clause bounds to generate nested obligations. For each bound marked with `#[diagnostic::traceable]`, the compiler annotates the resulting obligation to indicate that failures should not be suppressed. This annotation propagates through the obligation cause chain, ensuring that even if further nested obligations are generated while checking the traceable bound, the entire subtree is treated as significant.

The scoping decision reflects a design principle that diagnostic annotations should be as local and explicit as possible. Global or implicit annotations risk unintended consequences where library authors forget that marking one item affects many others. Local annotations make it immediately clear when reading code which constraints are marked traceable and allow fine-grained control over diagnostic behavior without increasing coupling between different parts of the codebase.

### 2.3 Interaction with Trait Resolution and Obligation Processing

The `#[diagnostic::traceable]` attribute does not change the trait resolution algorithm or affect which implementations are selected. It is purely a diagnostic annotation that modifies how errors are reported after trait resolution determines that an obligation cannot be satisfied. The trait solver processes traceable and non-traceable bounds identically during the solving phase; the difference emerges only in the error reporting phase.

When the trait solver generates an obligation from a where clause bound, it records in the obligation's metadata whether the bound was marked traceable. This metadata is stored in the obligation cause structure alongside existing information about where the obligation came from and why it was generated. During solving, this metadata has no effect. The solver attempts to fulfill all obligations regardless of whether they are traceable, and success or failure is determined entirely by whether matching implementations and bounds can be found.

After solving completes and errors are being processed for reporting, the error reporting system examines the traceable metadata to decide which errors to prioritize and report. Obligations that arose from traceable bounds are treated as leaf obligations for reporting purposes even if they have nested obligations that also failed. This ensures that when a traceable bound fails, the error message focuses on that bound specifically rather than reporting only its nested dependencies.

The interaction with the obligation forest in the current solver and the proof tree in the next-generation solver is straightforward. Both structures already maintain cause chains that record the derivation path for each obligation. The traceable metadata is simply additional information attached to specific nodes in these structures. Error reporting walks these structures to identify which failures to report, and the traceable metadata influences this decision without requiring changes to the core data structures.

This design ensures that adding or removing traceable annotations from library code cannot change the semantics of user code. Users can update library versions with different traceable annotations without worrying about behavioral changes. The attribute only affects what users see in error messages, not whether their code compiles successfully. Code that compiles before a library adds traceable annotations will continue to compile afterward, and code that fails to compile will still fail with more informative error messages.

### 2.4 Relationship to Existing Diagnostic Attributes

Rust already has several diagnostic attributes that influence error messages. The `#[rustc_on_unimplemented]` attribute customizes the message shown when a trait is not implemented. The `#[diagnostic::on_unimplemented]` attribute provides similar functionality with a more stable interface. The `#[diagnostic::do_not_recommend]` attribute, recently added, instructs the compiler to avoid recommending a particular implementation when reporting errors. These attributes focus on customizing message content, while `#[diagnostic::traceable]` focuses on controlling which errors are reported.

The traceable attribute complements rather than replaces these existing attributes. A trait bound can be both marked with a custom error message via `on_unimplemented` and marked as traceable. The traceable annotation ensures that the error is reported when the bound fails, while the on_unimplemented message customizes what that error says. Library authors should use both mechanisms together when appropriate, with traceable controlling visibility and on_unimplemented controlling message content.

The relationship with `do_not_recommend` is more subtle. That attribute tells the compiler to avoid recommending a particular implementation as a fix suggestion, typically used for fallback implementations that should not be surfaced to users. In contrast, traceable tells the compiler to ensure that failures of specific bounds are surfaced. These attributes operate at different stages of error reporting: do_not_recommend filters which implementations are mentioned in suggestions, while traceable filters which obligation failures are reported as primary errors.

The design deliberately uses the `diagnostic::` namespace introduced for stable diagnostic attributes rather than the `rustc_` namespace used for unstable compiler-internal attributes. This signals that traceable is intended as a stable, user-facing feature rather than a compiler implementation detail. It follows the precedent of `diagnostic::on_unimplemented` in providing a stable interface for features previously available only through unstable rustc attributes.

One important semantic difference from `rustc_on_unimplemented` is that traceable does not require the trait author to add the annotation. It can be added by implementation authors on their blanket implementations' where clauses. This enables library authors who are composing multiple traits to mark which compositions are semantically significant without requiring coordination with upstream trait definitions. The decentralization of control is intentional and important for the attribute to be practical in real-world codebases.

### 2.5 Forward Compatibility and Evolution Path

The design of `#[diagnostic::traceable]` provides clear paths for future evolution while maintaining backward compatibility. The attribute's semantics are defined in terms of influencing error reporting priorities, which allows implementation strategies to evolve as the compiler's error reporting improves. Future compiler versions can use traceable annotations more effectively without changing what the annotation means.

One natural evolution would be extending traceable to additional contexts beyond implementation where clauses. Function signatures, associated type bounds, and trait definition where clauses are all potential future targets. Each extension would need careful semantic definition, but the core principle of marking bounds as semantically significant would remain consistent. The current scoping to implementation where clauses establishes the pattern and gains implementation experience before expanding scope.

Another evolution path involves interaction with future language features. If Rust gains explicit support for blanket implementation priority or specialization, traceable markers could influence not just error reporting but also implementation selection. A traceable bound might indicate not only that failures should be reported but also that the bound represents a specialization requirement. This would require careful design to avoid changing existing code semantics.

The attribute could also gain parameters or flags to express more nuanced diagnostic policies. For example, `#[diagnostic::traceable(suggest_implementations)]` might indicate that when the bound fails, the compiler should actively suggest types that do satisfy the bound. Parameters like this extend the attribute's expressive power while maintaining backward compatibility, since existing uses without parameters continue to work with their original meaning.

Implementation wise, the attribute can start with conservative behavior that simply prevents suppression of marked bounds, then evolve to more sophisticated strategies like reordering error messages to present traceable failures first or generating specialized error message templates for common traceable bound patterns. These implementation improvements enhance the attribute's value without changing its semantic contract, allowing libraries to benefit from compiler improvements without code changes.

The forward compatibility design also considers deprecation scenarios. If future changes to Rust's trait system or error reporting make traceable annotations unnecessary because the compiler can automatically identify significant bounds, the attribute can be deprecated gradually. Existing annotations would become no-ops that the compiler ignores, generating warnings but not errors, allowing codebases to migrate away from the attribute at their own pace without breakage.

---

## Chapter 3: Detailed Specification

### Chapter Outline

This chapter provides the normative specification of `#[diagnostic::traceable]`, defining its syntax, semantic rules, and error conditions with the precision necessary for implementation. We specify the exact grammar for the attribute, define how it affects trait implementation checking semantics, explain how it applies when used in trait definitions (for future extension), cover its interaction with associated items, and enumerate the validation errors that the compiler must check. This chapter serves as the authoritative reference for implementing the feature.

### 3.1 Syntax and Grammar

The `#[diagnostic::traceable]` attribute follows Rust's standard attribute syntax. The full grammar production is:

```
TraceableAttribute := '#' '[' 'diagnostic' '::' 'traceable' ']'
```

The attribute is placed immediately before a trait bound in a where clause. Multiple traceable attributes on the same bound are allowed but have the same effect as a single attribute. The attribute takes no parameters in the initial version of this proposal. The attribute must appear on its own line or inline immediately before the bound it applies to.

Valid placements include before trait bounds in where clauses of implementation blocks. The implementation where clause may contain multiple bounds with independent traceable attributes. The following examples illustrate valid syntax:

```rust
impl<T> TraitA for T
where
    #[diagnostic::traceable]
    T: TraitB,
    T: TraitC,  // not marked traceable
{
    // implementation
}

impl<T> TraitA for T
where
    #[diagnostic::traceable] T: TraitB,
    #[diagnostic::traceable] T: TraitC,
{
    // implementation
}
```

The attribute is not valid in the following contexts and must be rejected with a compilation error: before the impl keyword, before the trait name in the impl header, inside the implementation body, on function signatures within the implementation, on associated type definitions, or on trait definitions themselves.

### 3.2 Semantic Rules for Trait Implementations

When the compiler checks whether a trait implementation applies to satisfy an obligation, it examines the implementation's where clause to generate nested obligations. For each where clause predicate marked with `#[diagnostic::traceable]`, the compiler creates an obligation with special metadata indicating that the obligation is traceable.

The traceable metadata propagates to all nested obligations generated while attempting to satisfy the traceable obligation. If satisfying a traceable bound requires checking additional predicates from blanket implementations or other sources, those recursive checks inherit the traceable property. This transitive propagation ensures that the entire subtree of constraints stemming from a traceable bound is preserved in error reporting.

When the trait solver determines that a traceable obligation cannot be satisfied, the error reporting system treats this as a leaf failure that must be reported directly to the user. The error message identifies which trait bound failed, which type failed to satisfy it, and why the failure occurred based on the obligation cause chain. The compiler does not suppress this error as transitive even if other related errors are being reported.

If multiple traceable bounds fail within the same implementation being checked, the compiler reports all such failures. The error messages are grouped to indicate that they all stem from checking the same implementation. The compiler does not attempt to identify a single root cause among multiple traceable failures, recognizing that each represents an independent semantic requirement.

The traceable metadata does not affect implementation selection priority or overlap resolution. If multiple implementations could apply to satisfy an obligation and differ in which bounds are marked traceable, this difference does not influence which implementation the compiler selects. The selection is based purely on specificity and overlap rules as defined by Rust's trait system semantics.

### 3.3 Semantic Rules for Trait Definitions

In the initial version of this proposal, `#[diagnostic::traceable]` cannot be applied to trait definitions. This section reserves the syntax and specifies the intended semantics for a future extension that would allow marking bounds on trait definitions as traceable.

If the attribute were allowed on trait definition where clauses, it would apply to all implementations of that trait. Any implementation that does not override the bound would inherit the traceable annotation. This would provide a way for trait authors to express that certain bounds are fundamental to the trait's contract and should always be reported clearly when unsatisfied.

The interaction with blanket implementations would need careful specification. A blanket implementation of a trait with traceable bounds in its definition would need to check those bounds, and failures would be reported according to the traceable annotation. However, the implementation could add additional traceable bounds of its own, and both sets of traceable bounds would be honored.

The primary challenge with allowing traceable on trait definitions is that it imposes a diagnostic policy that affects all implementations of the trait, which may be appropriate in some cases but overly restrictive in others. The current proposal defers this extension pending implementation experience with impl-level traceable annotations, allowing us to understand usage patterns before expanding scope.

### 3.4 Semantic Rules for Associated Types and Constants

Associated types and associated constants in trait implementations can have their own where clauses specifying bounds required for the associated item to be well-formed. In the initial version of this proposal, `#[diagnostic::traceable]` cannot be applied to these where clauses. This section reserves the syntax for potential future extension.

The challenge with associated item where clauses is that failures typically manifest when the associated item is used rather than when the trait implementation is checked. The causal chain for associated item errors is more complex, involving not just whether the implementation exists but also how the implementation's associated items are used. Supporting traceable annotations in this context would require careful design to avoid confusing error messages.

If extended to associated items, the semantics would specify that when normalizing an associated type or evaluating an associated constant, failures of traceable bounds on the associated item would be reported clearly even if they appear to be transitive failures from the perspective of the overall trait resolution. This could help in complex scenarios involving associated type families where the relationships between items are not obvious from type signatures alone.

### 3.5 Error Conditions and Validation

The compiler must validate traceable attributes and report errors for invalid usage. The following conditions are errors:

**Invalid Position Error**: If `#[diagnostic::traceable]` appears anywhere other than immediately before a trait bound in an implementation where clause, the compiler reports an error indicating that the attribute is only valid on trait bounds in where clauses. The error message includes the location where the attribute was found and suggests the correct placement.

**Parameter Error**: If the attribute is written with parameters or arguments such as `#[diagnostic::traceable(arg)]`, the compiler reports an error indicating that traceable does not accept parameters in the current version. The error message suggests removing the parameters. This validation reserves parameter syntax for future extensions.

**Duplicate Attribute Warning**: If multiple `#[diagnostic::traceable]` attributes appear before the same bound, the compiler emits a warning indicating that duplicate attributes have no additional effect. This is a warning rather than an error to maintain forward compatibility with potential future extensions that might give duplicate attributes meaning.

**Namespace Error**: If the attribute is written without the diagnostic namespace, such as `#[traceable]`, the compiler reports that traceable is not a recognized bare attribute and suggests using the full `#[diagnostic::traceable]` form. This prevents ambiguity with potential crate-level custom attributes using the same name.

**Scope Error**: If traceable is applied to bounds in contexts outside implementation where clauses, such as function signature where clauses or standalone where clauses, the compiler reports an error explaining that the current version only supports implementation where clauses and suggests alternative approaches for the specific context where the user attempted to use it.

These validation rules ensure that code using traceable attributes has clear semantics and prevent silent failures where the attribute is present but ignored. The errors guide users toward correct usage and provide forward compatibility by reserving syntax for future extensions.

---

## Chapter 4: Simple Examples with Vanilla Rust

### Chapter Outline

This chapter demonstrates the effects of `#[diagnostic::traceable]` through progressively more complex examples using standard Rust patterns without CGP-specific constructs. We begin with the simplest case of a blanket implementation with a single bound, show how traceable changes the error message, build up to multiple layers of delegation, explore interactions with multiple trait bounds, and provide side-by-side comparisons of error messages before and after applying traceable. The goal is to build intuition through concrete examples before examining more complex use cases.

### 4.1 Basic Blanket Implementation Without Traceable

Consider a simple trait for types that can be displayed in a table format:

```rust
trait TableDisplay {
    fn display_in_table(&self) -> String;
}
```

We provide a blanket implementation for all types that implement both Display and Debug:

```rust
impl<T> TableDisplay for T
where
    T: Display,
    T: Debug,
{
    fn display_in_table(&self) -> String {
        format!("Display: {}\nDebug: {:?}", self, self)
    }
}
```

Now a user attempts to use this trait with a custom type that implements Display but not Debug:

```rust
use std::fmt::{self, Display};

struct Person {
    name: String,
}

impl Display for Person {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.name)
    }
}

fn show_in_table<T: TableDisplay>(value: T) {
    println!("{}", value.display_in_table());
}

fn main() {
    let person = Person { name: "Alice".to_string() };
    show_in_table(person);
}
```

The current Rust compiler error message for this code is:

```
error[E0277]: `Person` doesn't implement `Debug`
  --> src/main.rs:20:5
   |
20 |     show_in_table(person);
   |     ------------- ^^^^^^ `Person` cannot be formatted using `{:?}`
   |     |
   |     required by a bound introduced by this call
   |
   = help: the trait `Debug` is not implemented for `Person`
   = note: add `#[derive(Debug)]` or manually implement `Debug`
note: required for `Person` to implement `TableDisplay`
  --> src/main.rs:6:12
   |
6  | impl<T> TableDisplay for T
   |            ^^^^^^^^^^^^^    ^
7  | where
8  |     T: Display,
9  |     T: Debug,
   |        ----- unsatisfied trait bound introduced here
```

This error message is actually quite good. The compiler identifies that Debug is not implemented and explains that this is required for TableDisplay. The note section traces through the blanket implementation where clause to show which bound is unsatisfied. For this simple case with a single level of delegation, the current heuristics work well.

### 4.2 Applying Traceable to Expose Hidden Constraints

Now consider a slightly more complex pattern where the blanket implementation delegates to a helper trait:

```rust
trait TableDisplay {
    fn display_in_table(&self) -> String;
}

trait DisplayHelpers {
    fn display_debug(&self) -> String;
    fn display_pretty(&self) -> String;
}

impl<T: Debug + Display> DisplayHelpers for T {
    fn display_debug(&self) -> String {
        format!("{:?}", self)
    }
    
    fn display_pretty(&self) -> String {
        format!("{}", self)
    }
}

impl<T> TableDisplay for T
where
    T: DisplayHelpers,
{
    fn display_in_table(&self) -> String {
        format!("Debug: {}\nPretty: {}", 
            self.display_debug(),
            self.display_pretty())
    }
}
```

When a user tries to use this with a type that implements Display but not Debug:

```rust
fn main() {
    let person = Person { name: "Alice".to_string() };
    show_in_table(person);
}
```

The compiler error becomes less helpful:

```
error[E0277]: the trait bound `Person: DisplayHelpers` is not satisfied
  --> src/main.rs:35:5
   |
35 |     show_in_table(person);
   |     ------------- ^^^^^^ the trait `DisplayHelpers` is not implemented for `Person`
   |     |
   |     required by a bound introduced by this call
   |
note: required for `Person` to implement `TableDisplay`
  --> src/main.rs:21:12
   |
21 | impl<T> TableDisplay for T
   |            ^^^^^^^^^^^^^    ^
22 | where
23 |     T: DisplayHelpers,
   |        -------------- unsatisfied trait bound introduced here
```

The error correctly identifies that DisplayHelpers is not implemented, but it does not explain why DisplayHelpers is not implemented. A user unfamiliar with the library must now examine the DisplayHelpers trait and find the blanket implementation to discover that Debug is the missing piece. The information about Debug being required is lost in the error message.

Now we apply `#[diagnostic::traceable]` to the Debug bound in the DisplayHelpers implementation:

```rust
impl<T> DisplayHelpers for T
where
    #[diagnostic::traceable]
    T: Debug,
    T: Display,
{
    fn display_debug(&self) -> String {
        format!("{:?}", self)
    }
    
    fn display_pretty(&self) -> String {
        format!("{}", self)
    }
}
```

With this annotation, the error message becomes:

```
error[E0277]: `Person` doesn't implement `Debug`
  --> src/main.rs:35:5
   |
35 |     show_in_table(person);
   |     ------------- ^^^^^^ `Person` cannot be formatted using `{:?}`
   |     |
   |     required by a bound introduced by this call
   |
   = help: the trait `Debug` is not implemented for `Person`
   = note: add `#[derive(Debug)]` or manually implement `Debug`
note: required for `Person` to implement `DisplayHelpers`
  --> src/main.rs:11:12
   |
11 | impl<T> DisplayHelpers for T
   |            ^^^^^^^^^^^^^^    ^
12 | where
13 |     #[diagnostic::traceable]
14 |     T: Debug,
   |        ----- unsatisfied trait bound introduced here
note: required for `Person` to implement `TableDisplay`
  --> src/main.rs:21:12
   |
21 | impl<T> TableDisplay for T
   |            ^^^^^^^^^^^^^    ^
22 | where
23 |     T: DisplayHelpers,
   |        -------------- required by this bound in `TableDisplay`
```

The traceable annotation causes the compiler to report the Debug bound failure as the primary error, while the notes trace upward through the delegation chain. The user immediately sees that Debug is missing and understands the full path from their code through TableDisplay to the actual requirement.

### 4.3 Multiple Layers of Delegation

Let's extend the example to three levels of delegation to see how traceable handles deeper chains:

```rust
trait TableDisplay {
    fn display_in_table(&self) -> String;
}

trait DisplayHelpers {
    fn display_debug(&self) -> String;
}

trait DebugProvider {
    fn provide_debug(&self) -> String;
}

impl<T> DebugProvider for T
where
    #[diagnostic::traceable]
    T: Debug,
{
    fn provide_debug(&self) -> String {
        format!("{:?}", self)
    }
}

impl<T> DisplayHelpers for T
where
    T: DebugProvider,
{
    fn display_debug(&self) -> String {
        self.provide_debug()
    }
}

impl<T> TableDisplay for T
where
    T: DisplayHelpers,
{
    fn display_in_table(&self) -> String {
        format!("Value: {}", self.display_debug())
    }
}
```

Without traceable on the Debug bound, attempting to use this with a non-Debug type produces an error mentioning either DisplayHelpers or DebugProvider, depending on the compiler's heuristics. The user must trace through three trait definitions to discover that Debug is the root requirement.

With traceable on the Debug bound in DebugProvider's implementation, the error directly reports that Debug is not implemented and provides notes showing the full chain: Debug is required for DebugProvider, which is required for DisplayHelpers, which is required for TableDisplay. The user understands the complete picture from a single error message.

### 4.4 Interaction with Multiple Trait Bounds

Consider a blanket implementation with multiple where clause bounds, some marked traceable and others not:

```rust
trait Processor {
    fn process(&self) -> String;
}

trait Logger {
    fn log(&self);
}

trait Provider {
    fn provide(&self) -> i32;
}

impl<T> Processor for T
where
    #[diagnostic::traceable]
    T: Logger,
    T: Provider,
    T: Clone,
{
    fn process(&self) -> String {
        self.log();
        format!("Processed: {}", self.provide())
    }
}
```

If a type implements Provider and Clone but not Logger:

```rust
struct Worker {
    value: i32,
}

impl Provider for Worker {
    fn provide(&self) -> i32 {
        self.value
    }
}

impl Clone for Worker {
    fn clone(&self) -> Self {
        Worker { value: self.value }
    }
}

fn use_processor<T: Processor>(t: T) {
    println!("{}", t.process());
}

fn main() {
    let worker = Worker { value: 42 };
    use_processor(worker);
}
```

The error message focuses on the missing Logger implementation because it is marked traceable:

```
error[E0277]: the trait bound `Worker: Logger` is not satisfied
  --> src/main.rs:43:5
   |
43 |     use_processor(worker);
   |     ------------- ^^^^^^ the trait `Logger` is not implemented for `Worker`
   |     |
   |     required by a bound introduced by this call
   |
note: required for `Worker` to implement `Processor`
  --> src/main.rs:14:12
   |
14 | impl<T> Processor for T
   |            ^^^^^^^^^    ^
15 | where
16 |     #[diagnostic::traceable]
17 |     T: Logger,
   |        ------ unsatisfied trait bound introduced here
```

The error prioritizes the traceable bound even though Provider and Clone were satisfied. If multiple traceable bounds are unsatisfied, all are reported with equal priority.

### 4.5 Comparison of Error Messages Before and After

To summarize the impact, consider a two-level delegation chain without traceable:

**Before (without traceable):**
```
error[E0277]: the trait bound `Person: DisplayHelpers` is not satisfied
```

The error identifies the intermediate trait but provides no information about why it is not satisfied. The user must manually investigate DisplayHelpers to discover the actual requirements.

**After (with traceable):**
```
error[E0277]: `Person` doesn't implement `Debug`
  = note: required for `Person` to implement `DisplayHelpers`
  = note: required for `Person` to implement `TableDisplay`
```

The error identifies the root cause (Debug not implemented) and traces upward through the delegation chain. The user immediately understands both what is missing and why it is needed.

For deeper chains, the improvement is more dramatic. Without traceable, a five-level chain might report a failure at level three, forcing the user to manually investigate two more levels. With traceable, the error reports the leaf requirement and shows all five levels of context, eliminating the need for manual investigation.

The key insight is that traceable does not make error messages longer in ways that obscure information. It redirects the compiler's attention to report the genuinely useful information (what capability is actually missing) while maintaining context about why that capability is needed. The result is that error messages are both more complete and more actionable.

---

## Chapter 5: Application to Context-Generic Programming

### Chapter Outline

This chapter examines how `#[diagnostic::traceable]` addresses the specific error message challenges in Context-Generic Programming patterns. We begin with a brief high-level overview of CGP for readers unfamiliar with the pattern, then demonstrate how provider-consumer delegation chains produce poor error messages without traceable and how the attribute improves them. We show that traceable eliminates the need for workaround traits that exist solely for diagnostic purposes. We then explore how traceable helps with higher-order providers and deeply nested dependencies. The chapter concludes with complete before-and-after error message examples for realistic CGP code.

### 5.1 Brief Overview of CGP Patterns

Context-Generic Programming is a code organization pattern that uses Rust's trait system to achieve high modularity and reusability. The core idea is to separate capability definitions, capability implementations, and capability composition. A context type aggregates capabilities through trait implementations, while provider types encapsulate the logic for specific capabilities. Blanket trait implementations wire providers to contexts based on delegation relationships declared in the context type.

A simple CGP pattern involves three elements: a consumer trait that defines a capability interface, a provider trait that defines how to implement that capability, and a blanket implementation that implements the consumer trait for any context that properly delegates to a provider. For example, a consumer trait might define area calculation, a provider trait defines how to compute area for different shape types, and the blanket implementation connects the two.

The delegation is expressed through trait bounds in the blanket implementation's where clause. The context type must implement certain getters to access fields, must designate which provider to use for each capability, and the provider must implement its provider trait for that specific context. When any of these requirements is unsatisfied, the consumer trait cannot be implemented for the context, and the compiler reports an error.

The modularity benefit is that capabilities can be mixed and matched independently. A context can choose different providers for different capabilities, providers can be reused across different contexts, and new providers can be added without modifying existing code. The cost is that the blanket implementations create dependency chains where a missing field on the context type manifests as a trait bound failure deep within provider trait implementations.

### 5.2 Provider-Consumer Delegation Chains

Consider a concrete CGP example using traits derived from the attached example code:

```rust
trait CanCalculateArea {
    fn area(&self) -> f64;
}

trait HasRectangleFields {
    fn width(&self) -> f64;
    fn height(&self) -> f64;
}

trait AreaCalculator<Context> {
    fn area(context: &Context) -> f64;
}

struct RectangleArea;

impl<Context> AreaCalculator<Context> for RectangleArea
where
    Context: HasRectangleFields,
{
    fn area(context: &Context) -> f64 {
       context.width() * context.height()
    }
}

impl<Context, Provider> CanCalculateArea for Context
where
    Context: DelegateComponent<AreaCalculatorComponent, Delegate = Provider>,
    Provider: AreaCalculator<Context>,
{
    fn area(&self) -> f64 {
        Provider::area(self)
    }
}
```

The delegation chain flows: CanCalculateArea requires a Provider that implements AreaCalculator, which in turn requires the Context to implement HasRectangleFields. If a Rectangle type uses RectangleArea as its provider but is missing the height field:

```rust
struct Rectangle {
    width: f64,
    // height field missing
}

impl DelegateComponent<AreaCalculatorComponent> for Rectangle {
    type Delegate = RectangleArea;
}
```

Without traceable, the error message might report:

```
error[E0277]: the trait bound `Rectangle: HasRectangleFields` is not satisfied
```

This identifies HasRectangleFields as unimplemented but does not explain which specific field is missing. The user must examine the HasRectangleFields trait definition, discover it requires width and height methods, trace those back to field requirements, and check which field their struct is missing.

Now we apply traceable to the HasRectangleFields bound:

```rust
impl<Context> AreaCalculator<Context> for RectangleArea
where
    #[diagnostic::traceable]
    Context: HasRectangleFields,
{
    fn area(context: &Context) -> f64 {
        context.width() * context.height()
    }
}
```

The error message improves to:

```
error[E0277]: the trait bound `Rectangle: HasField<"height">` is not satisfied
  = note: required for `Rectangle` to implement `HasRectangleFields`
  = note: required for `RectangleArea` to implement `AreaCalculator<Rectangle>`
  = note: required for `Rectangle` to implement `CanCalculateArea`
```

The traceable annotation causes the compiler to drill down through HasRectangleFields to identify that the specific problem is the missing height field. The notes trace back up through the delegation chain, showing the user exactly which field is missing and why it is needed for area calculation.

### 5.3 Eliminating the Need for Workaround Traits

Some CGP codebases introduce marker traits whose sole purpose is to improve error messages. These traits have explicit implementations that must be manually defined, ensuring that when they are missing, the compiler reports this as a leaf failure. The pattern looks like:

```rust
// Workaround trait for diagnostics only
trait HasAreaProvider {}

impl HasAreaProvider for Rectangle {}

impl<Context, Provider> CanCalculateArea for Context
where
    Context: HasAreaProvider,  // Forces better error messages
    Context: DelegateComponent<AreaCalculatorComponent, Delegate = Provider>,
    Provider: AreaCalculator<Context>,
{
    fn area(&self) -> f64 {
        Provider::area(self)
    }
}
```

The HasAreaProvider trait serves no semantic purpose. It exists only because the compiler treats its absence as a leaf failure and reports it clearly. This approach pollutes the API with diagnostic-only traits, requires boilerplate implementations, and does not scale to complex dependency patterns.

With `#[diagnostic::traceable]`, these workaround traits become unnecessary:

```rust
impl<Context, Provider> CanCalculateArea for Context
where
    #[diagnostic::traceable]
    Context: DelegateComponent<AreaCalculatorComponent, Delegate = Provider>,
    #[diagnostic::traceable]
    Provider: AreaCalculator<Context>,
{
    fn area(&self) -> f64 {
        Provider::area(self)
    }
}
```

Marking the actual semantic requirements as traceable ensures they are reported clearly without introducing artificial marker traits. The DelegateComponent and AreaCalculator bounds represent genuine requirements of the blanket implementation, and traceable communicates to the compiler that failures of these bounds should be prioritized in error messages.

This simplifies the codebase by removing diagnostic-only traits and their associated implementations. It reduces the cognitive load on users who no longer need to understand why certain traits exist when they serve no semantic role. It scales naturally to complex patterns where multiple nested requirements all need to be visible in error messages.

### 5.4 Higher-Order Providers and Nested Dependencies

CGP patterns often involve higher-order providers where one provider wraps another provider. Consider a scaled area calculator that multiplies the result from an inner area calculator by a scale factor:

```rust
struct ScaledArea<InnerCalculator>(PhantomData<InnerCalculator>);

impl<Context, InnerCalculator> AreaCalculator<Context> for ScaledArea<InnerCalculator>
where
    Context: HasScaleFactor,
    InnerCalculator: AreaCalculator<Context>,
{
    fn area(context: &Context) -> f64 {
        context.scale_factor() * InnerCalculator::area(context)
    }
}
```

A Rectangle using ScaledArea<RectangleArea> requires both a scale_factor field (for ScaledArea) and width/height fields (for RectangleArea). If the height field is missing, the error manifests deep within the nested provider chain.

Without traceable, the error might report:

```
error[E0277]: the trait bound `RectangleArea: AreaCalculator<Rectangle>` is not satisfied
```

This identifies that RectangleArea cannot provide area calculation but does not explain why. The user must examine RectangleArea's implementation to discover it requires HasRectangleFields, then examine HasRectangleFields to discover the specific field requirement.

With traceable applied to both the inner and outer provider bounds:

```rust
impl<Context, InnerCalculator> AreaCalculator<Context> for ScaledArea<InnerCalculator>
where
    #[diagnostic::traceable]
    Context: HasScaleFactor,
    #[diagnostic::traceable]
    InnerCalculator: AreaCalculator<Context>,
{
    fn area(context: &Context) -> f64 {
        context.scale_factor() * InnerCalculator::area(context)
    }
}

impl<Context> AreaCalculator<Context> for RectangleArea
where
    #[diagnostic::traceable]
    Context: HasRectangleFields,
{
    fn area(context: &Context) -> f64 {
        context.width() * context.height()
    }
}
```

The error message becomes:

```
error[E0277]: the trait bound `Rectangle: HasField<"height">` is not satisfied
  = note: required for `Rectangle` to implement `HasRectangleFields`
  = note: required for `RectangleArea` to implement `AreaCalculator<Rectangle>`
  = note: required for `ScaledArea<RectangleArea>` to implement `AreaCalculator<Rectangle>`
  = note: required for `Rectangle` to implement `CanCalculateArea`
```

The traceable annotations cause the compiler to traverse the entire nested provider chain and identify the leaf requirement. The notes show the complete path from the missing height field through RectangleArea, ScaledArea, and finally to CanCalculateArea. This eliminates the need for the user to manually trace through multiple levels of provider composition.

### 5.5 Complete Error Message Transformation Examples

To see the full impact, consider a realistic CGP scenario with multiple capabilities and dependencies. A Rectangle context that can calculate density, where density depends on area and mass:

```rust
trait CanCalculateDensity {
    fn density(&self) -> f64;
}

trait DensityCalculator<Context> {
    fn density(context: &Context) -> f64;
}

struct DensityFromMass;

impl<Context> DensityCalculator<Context> for DensityFromMass
where
    Context: CanCalculateArea + HasMass,
{
    fn density(context: &Context) -> f64 {
        context.mass() / context.area()
    }
}

impl<Context, Provider> CanCalculateDensity for Context
where
    Context: DelegateComponent<DensityCalculatorComponent, Delegate = Provider>,
    Provider: DensityCalculator<Context>,
{
    fn density(&self) -> f64 {
        Provider::density(self)
    }
}
```

With a Rectangle that is missing the height field:

```rust
struct Rectangle {
    mass: f64,
    width: f64,
    // height missing
}
```

**Error message without traceable:**

```
error[E0277]: the trait bound `Rectangle: CanCalculateArea` is not satisfied
  --> src/main.rs:XX:XX
   |
   | impl<Context, Provider> CanCalculateDensity for Context
   |                         ^^^^^^^^^^^^^^^^^^
   |
   = help: the trait `CanCalculateArea` is not implemented for `Rectangle`
```

This identifies that CanCalculateArea is not implemented but provides no information about why. The user must manually trace through the area calculation delegation chain to discover the missing field.

**Error message with traceable:**

```rust
impl<Context> AreaCalculator<Context> for RectangleArea
where
    #[diagnostic::traceable]
    Context: HasRectangleFields,
{
    fn area(context: &Context) -> f64 {
        context.width() * context.height()
    }
}

impl<Context> DensityCalculator<Context> for DensityFromMass
where
    #[diagnostic::traceable]
    Context: CanCalculateArea,
{
    fn density(context: &Context) -> f64 {
        context.mass() / context.area()
    }
}
```

The error becomes:

```
error[E0277]: the trait bound `Rectangle: HasField<"height">` is not satisfied
  --> src/main.rs:XX:XX
   |
   = help: the field `height` is not defined on `Rectangle`
   = note: required for `Rectangle` to implement `HasRectangleFields`
   = note: required for `RectangleArea` to implement `AreaCalculator<Rectangle>`
   = note: required for `Rectangle` to implement `CanCalculateArea`
   = note: required for `DensityFromMass` to implement `DensityCalculator<Rectangle>`
   = note: required for `Rectangle` to implement `CanCalculateDensity`
```

The traceable annotations at each level ensure the error traces all the way to the root cause while showing the complete dependency path. The user immediately understands that adding the height field will fix the problem and sees exactly why that field is needed for density calculation.

---

## Chapter 6: Application to Other Patterns

### Chapter Outline

This chapter explores how `#[diagnostic::traceable]` benefits patterns beyond CGP, demonstrating that the attribute addresses a general problem in Rust's trait system rather than being narrowly targeted at a single use case. We examine type-level computation patterns involving associated types, builder patterns with compile-time validation, effect systems that encode capabilities in the type system, and zero-cost abstraction patterns using phantom types. For each pattern, we show how traceable improves error messages and enables more sophisticated API designs without sacrificing diagnostic quality.

### 6.1 Type-Level Computation and Associated Types

Rust's type system enables computation at compile time through associated types and trait bounds. Libraries can encode invariants and relationships between types that the compiler verifies. However, when these type-level computations involve multiple steps, error messages often report intermediate failures rather than the root cause.

Consider a type-level natural number arithmetic library:

```rust
trait Nat {
    type Value: Nat;
}

struct Zero;
struct Succ<N: Nat>(PhantomData<N>);

impl Nat for Zero {
    type Value = Zero;
}

impl<N: Nat> Nat for Succ<N> {
    type Value = Succ<N>;
}

trait Add<Rhs: Nat> {
    type Output: Nat;
}

impl<N: Nat> Add<Zero> for N {
    type Output = N;
}

impl<N: Nat, M: Nat> Add<Succ<M>> for N
where
    N: Add<M>,
    <N as Add<M>>::Output: Nat,
{
    type Output = Succ<<N as Add<M>>::Output>;
}
```

When a user attempts an invalid type-level computation:

```rust
fn add_three<N: Nat>() -> <N as Add<Succ<Succ<Succ<Zero>>>>>::Output
where
    N: Add<Succ<Succ<Succ<Zero>>>>,
{
    unimplemented!()
}
```

If the trait bound resolution fails at some intermediate step, the error might report:

```
error[E0277]: the trait bound `<N as Add<Succ<Zero>>>::Output: Nat` is not satisfied
```

This reports an intermediate type that appears during the recursive addition but does not clearly indicate what the user needs to fix. With traceable applied to the Nat bound:

```rust
impl<N: Nat, M: Nat> Add<Succ<M>> for N
where
    N: Add<M>,
    #[diagnostic::traceable]
    <N as Add<M>>::Output: Nat,
{
    type Output = Succ<<N as Add<M>>::Output>;
}
```

The error becomes more informative about which specific associated type constraint failed and why it matters for the overall computation. The traceable annotation ensures that failures in well-formedness constraints for associated types are not obscured by reporting only higher-level trait bound failures.

### 6.2 Builder Patterns with Compile-Time Validation

The typestate pattern uses types to encode object states, ensuring that methods can only be called when the object is in an appropriate state. Builder patterns leverage this to verify at compile time that required fields have been set before building the final object:

```rust
struct Builder<FieldAState, FieldBState> {
    field_a: Option<i32>,
    field_b: Option<String>,
    _marker: PhantomData<(FieldAState, FieldBState)>,
}

struct Set;
struct Unset;

trait CanBuild {
    type Output;
    fn build(self) -> Self::Output;
}

impl<FA, FB> Builder<FA, FB> {
    fn set_a(mut self, val: i32) -> Builder<Set, FB> {
        self.field_a = Some(val);
        Builder {
            field_a: self.field_a,
            field_b: self.field_b,
            _marker: PhantomData,
        }
    }
}

impl CanBuild for Builder<Set, Set> {
    type Output = FinalObject;
    
    fn build(self) -> FinalObject {
        FinalObject {
            field_a: self.field_a.unwrap(),
            field_b: self.field_b.unwrap(),
        }
    }
}

fn finalize<B: CanBuild>(builder: B) -> B::Output {
    builder.build()
}
```

If a user attempts to build with missing fields:

```rust
let builder = Builder::new().set_a(42);
let obj = finalize(builder);  // field_b not set
```

The error might report:

```
error[E0277]: the trait bound `Builder<Set, Unset>: CanBuild` is not satisfied
```

This tells the user that the builder cannot be built but does not explain which field is missing. With more complex builders involving many fields and validation requirements, the error provides little guidance.

If the CanBuild trait used trait bounds to check field states:

```rust
trait FieldASet {}
trait FieldBSet {}

impl FieldASet for Set {}
impl FieldBSet for Set {}

impl<FA: FieldASet, FB: FieldBSet> CanBuild for Builder<FA, FB> {
    type Output = FinalObject;
    
    fn build(self) -> FinalObject {
        FinalObject {
            field_a: self.field_a.unwrap(),
            field_b: self.field_b.unwrap(),
        }
    }
}
```

Without traceable, the error still reports CanBuild not satisfied. With traceable:

```rust
impl<FA, FB> CanBuild for Builder<FA, FB>
where
    #[diagnostic::traceable]
    FA: FieldASet,
    #[diagnostic::traceable]
    FB: FieldBSet,
{
    type Output = FinalObject;
    
    fn build(self) -> FinalObject {
        FinalObject {
            field_a: self.field_a.unwrap(),
            field_b: self.field_b.unwrap(),
        }
    }
}
```

The error becomes:

```
error[E0277]: the trait bound `Unset: FieldBSet` is not satisfied
  = note: field B has not been set on the builder
  = note: required for `Builder<Set, Unset>` to implement `CanBuild`
```

The traceable annotation ensures the error identifies which specific field is missing rather than just reporting that the builder is incomplete.

### 6.3 Effect Systems and Capability-Based Designs

Effect systems use the type system to track capabilities or side effects that code is allowed to perform. A function's type signature includes bounds representing permissions it requires. These systems often use blanket implementations to compose capabilities:

```rust
trait CanReadFile {
    fn read_file(&self, path: &str) -> String;
}

trait CanWriteFile {
    fn write_file(&self, path: &str, content: &str);
}

trait CanAccessNetwork {
    fn fetch_url(&self, url: &str) -> String;
}

trait CanProcessData: CanReadFile + CanWriteFile {}

impl<T> CanProcessData for T
where
    T: CanReadFile + CanWriteFile,
{
}

fn process<Context: CanProcessData>(ctx: &Context) {
    let data = ctx.read_file("input.txt");
    ctx.write_file("output.txt", &data);
}
```

A context that has read but not write capabilities fails to implement CanProcessData. Without traceable, the error reports:

```
error[E0277]: the trait bound `ReadOnlyContext: CanProcessData` is not satisfied
```

With traceable applied to the CanWriteFile bound:

```rust
impl<T> CanProcessData for T
where
    T: CanReadFile,
    #[diagnostic::traceable]
    T: CanWriteFile,
{
}
```

The error becomes:

```
error[E0277]: the trait bound `ReadOnlyContext: CanWriteFile` is not satisfied
  = help: the context does not have write file capabilities
  = note: required for `ReadOnlyContext` to implement `CanProcessData`
```

The traceable annotation identifies the specific missing capability rather than just reporting the higher-level trait failure. For effect systems with many capabilities and complex composition rules, this significantly improves error clarity.

### 6.4 Phantom Type Constraints and Zero-Cost Abstractions

Phantom type parameters enable encoding additional type system information without runtime cost. Libraries use phantom types to enforce constraints that the compiler can verify statically. When these constraints are implemented through blanket traits, error messages can obscure the actual requirements:

```rust
struct Length<Unit>(f64, PhantomData<Unit>);

struct Meters;
struct Feet;

trait CompatibleUnits<Rhs> {}

impl CompatibleUnits<Meters> for Meters {}
impl CompatibleUnits<Feet> for Feet {}

impl<U1, U2> Add for Length<U1>
where
    U1: CompatibleUnits<U2>,
{
    type Output = Length<U1>;
    
    fn add(self, other: Length<U2>) -> Self::Output {
        Length(self.0 + other.0, PhantomData)
    }
}
```

Attempting to add lengths with incompatible units:

```rust
let a = Length::<Meters>(10.0, PhantomData);
let b = Length::<Feet>(5.0, PhantomData);
let c = a + b;  // error: incompatible units
```

Without traceable:

```
error[E0277]: the trait bound `Meters: CompatibleUnits<Feet>` is not satisfied
```

With traceable:

```rust
impl<U1, U2> Add<Length<U2>> for Length<U1>
where
    #[diagnostic::traceable]
    U1: CompatibleUnits<U2>,
{
    type Output = Length<U1>;
    
    fn add(self, other: Length<U2>) -> Self::Output {
        Length(self.0 + other.0, PhantomData)
    }
}
```

The error is essentially the same in this simple case, but the traceable annotation ensures that even if CompatibleUnits grows to have its own delegation chain, the unit incompatibility remains visible as the root cause. For more complex type-level constraints involving multiple phantom parameters and validation layers, traceable ensures that errors identify which specific constraint failed.

---

## Chapter 7: Implementation Strategy

### Chapter Outline

This chapter provides detailed guidance for implementing `#[diagnostic::traceable]` in the Rust compiler. We describe how to extend obligation cause tracking to preserve traceable metadata, how to modify error filtering logic to honor traceable markers, how to implement proof tree analysis for the new solver, how to handle obligation forests in the old solver, and how to generate improved error messages. The implementation strategy is designed to minimize changes to the trait resolution core while maximizing improvements to diagnostic output.

### 7.1 Obligation Cause Enhancement

The foundation of the implementation is extending the obligation cause tracking system to record when an obligation arises from a traceable bound. The `ObligationCause` structure in rustc_middle/src/traits/mod.rs contains an `ObligationCauseCode` enum that encodes why an obligation was generated. We add a new variant or flag to this enum indicating that the obligation comes from a traceable bound.

When processing an implementation's where clauses during trait resolution, the compiler examines each predicate to determine if it has a traceable attribute. For predicates marked traceable, the generated obligation's cause code is annotated with traceable metadata. This annotation is distinct from the existing cause codes and can be combined with them, so an obligation can be both an impl-derived obligation and a traceable obligation.

The traceable metadata must propagate through derived obligations. When the trait solver generates nested obligations while attempting to satisfy a traceable obligation, those nested obligations should inherit the traceable property. This is implemented in the `derived_cause` and related functions that construct cause chains. The derivation logic checks whether the parent obligation is traceable and if so, marks the child obligation as traceable as well.

The propagation is transitive but scoped. A traceable bound T: TraitA generates traceable obligations for all constraints needed to satisfy TraitA. If satisfying TraitA requires T: TraitB through a blanket implementation, the TraitB obligation is traceable. If TraitB's implementation has non-traceable bounds, those bounds generate non-traceable nested obligations unless they themselves are marked traceable. This ensures that traceable annotations compose in a predictable way.

The implementation must handle both the current trait solver's obligation forest and the next-generation solver's proof trees. For the obligation forest, traceable metadata is stored in the `PendingPredicateObligation` structure. For proof trees, the metadata is stored in goal metadata that the proof tree visitor can access. Both representations must preserve the information through all stages of solving and make it available to error reporting.

### 7.2 Error Filtering Modifications

The error reporting layer in rustc_trait_selection/src/error_reporting/traits/mod.rs implements filtering heuristics that determine which errors to report. The key function is `report_fulfillment_errors`, which processes a list of failed obligations and selects which ones to present to the user. The implementation must modify this filtering to prioritize traceable obligations.

The existing filtering logic identifies leaf obligations by checking whether an obligation has no further nested obligations that also failed. For traceable obligations, this check is bypassed. A traceable obligation is always considered a candidate for reporting regardless of whether it has failed nested obligations. This ensures that marking a bound as traceable prevents the compiler from suppressing it as a transitive failure.

When multiple errors are being reported for the same overall failure, traceable errors are prioritized. The implementation can assign higher priority scores to traceable obligations during the filtering process. If both traceable and non-traceable obligations fail in the same context, the traceable obligations are reported first or instead of non-traceable ones, depending on the severity and relationship between the failures.

The filtering logic must also handle the case where multiple traceable obligations fail independently. When this occurs, all traceable failures should be reported rather than attempting to identify a single root cause. The error messages should clearly indicate that multiple independent requirements are unsatisfied. This may require grouping traceable errors by the implementation they come from to avoid presenting them as unrelated.

The implementation should preserve backward compatibility for code without traceable annotations. The filtering heuristics continue to work as before for non-traceable obligations. The changes only affect how traceable obligations are treated, ensuring that adding traceable annotations to libraries does not degrade error messages for code that does not use those libraries or for implementations that have not been annotated.

### 7.3 Proof Tree Analysis in the New Solver

The next-generation trait solver in rustc_trait_selection/src/solve uses proof trees that record how goals were evaluated. The `BestObligation` visitor in derive_errors.rs traverses proof trees to identify leaf obligations for error reporting. The implementation must extend this visitor to recognize and prioritize traceable goals.

The visitor currently examines goals recursively to find goals that have no successful candidates. When a goal fails because its candidates all have unsatisfied requirements, the visitor continues to the nested goals to find where the chain ultimately breaks. For traceable goals, this traversal should stop and report the traceable goal itself rather than descending further.

The proof tree structure contains goal metadata that can store the traceable flag. When the solver generates a goal from a traceable bound, it marks the goal as traceable in this metadata. The BestObligation visitor checks this metadata when deciding whether to continue traversing or to report the current goal as the best obligation. A traceable goal with a failing result becomes a reporting candidate even if it has nested failing goals.

The visitor must handle the case where multiple paths through the proof tree contain traceable goals. When the same top-level goal can fail through different candidate implementations that each have different traceable bounds, all such failures should be collected. The error reporting system can then present multiple possible explanations for the failure, indicating that satisfying any of the traceable requirements would allow progress.

The implementation should also handle the interaction between traceable goals and goals marked with do_not_recommend. A goal that is both traceable and not recommended might appear contradictory, but the correct behavior is to report the goal but suppress it from fix suggestions. The traceable annotation is about visibility while do_not_recommend is about suggestion quality.

### 7.4 Obligation Forest Handling in the Old Solver

The current trait solver maintains an obligation forest in rustc_data_structures/src/obligation_forest/mod.rs that tracks pending obligations and their relationships. When errors are converted from this forest for reporting, the implementation must check traceable metadata and adjust filtering accordingly.

The obligation forest structure already maintains parent-child relationships between obligations. The implementation adds traceable metadata to obligation nodes and ensures this metadata is preserved as obligations are processed. When an obligation is marked traceable, this information is recorded in the node structure and persists through all forest operations.

The `to_errors` method that converts failed obligations into error structures must be modified to include traceable metadata in the generated errors. This allows the error reporting layer to know which errors come from traceable bounds. The metadata is attached to the `FulfillmentError` structure so that filtering logic can access it.

The filtering in the old solver is less sophisticated than in the new solver, primarily identifying which obligations are independent versus transitive. For traceable obligations, the implementation marks them as independent regardless of their position in the obligation tree. This prevents them from being filtered out as consequences of other failures.

The old solver implementation should be conservative in applying traceable logic, ensuring that the changes do not destabilize existing behavior. Since the trait system is in the process of transitioning to the new solver, the old solver implementation can be simpler and focus on correctness rather than optimality. The primary goal is to ensure that traceable annotations provide some benefit even when the old solver is used.

### 7.5 Error Message Generation and Formatting

The final stage of implementation is generating clear error messages that take advantage of traceable information. The error message formatting in rustc_trait_selection/src/error_reporting/traits/fulfillment_errors.rs must be enhanced to present traceable obligation failures in a way that highlights the root cause while preserving context.

For traceable obligation failures, the error message structure should emphasize the traceable bound itself as the primary error. The message begins by clearly stating which trait bound was not satisfied, using the same clarity as leaf obligation errors currently receive. This ensures users immediately see what specific requirement failed.

The notes section of the error message traces upward through the obligation cause chain to show how the traceable bound relates to the user's code. Each level of the chain is presented as a separate note, showing which trait implementation required the traceable bound, which higher-level trait that implementation was satisfying, and ultimately what user code triggered the requirement. This traces from the specific requirement up to the usage context.

When multiple traceable bounds fail for the same implementation, the error message presents them in a grouped fashion. A primary error identifies that the implementation cannot be selected, followed by multiple sub-errors or notes explaining that several traceable bounds are unsatisfied. This groups related failures while ensuring each traceable requirement is visible.

The implementation should leverage existing error message customization mechanisms like `#[rustc_on_unimplemented]` when present. If a traceable trait bound has a custom error message annotation, that message should be used when reporting the traceable bound failure. The custom message provides domain-specific context while the traceable annotation ensures the bound is reported in the first place.

Error messages should remain concise when traceable obligations are satisfied but other failures occur. If a deeply nested chain includes both traceable and non-traceable obligations, and only non-traceable obligations fail, the message should not become verbose just because traceable annotations exist in the chain. The annotations only affect reporting when the traceable obligations themselves fail.

---

## Chapter 8: Impact Analysis and Considerations

### Chapter Outline

This chapter analyzes the broader impacts of introducing `#[diagnostic::traceable]` into the Rust ecosystem. We examine performance implications for compilation, assess trade-offs in error message verbosity, discuss interaction with future language features, provide a migration path for existing code, and offer guidance on documentation and proper usage. The goal is to ensure the feature integrates smoothly into Rust's evolution while providing clear value to users.

### 8.1 Performance Implications

The `#[diagnostic::traceable]` attribute has minimal performance impact on compilation because it only affects error reporting, which occurs after trait resolution has already completed. The trait solving process itself remains unchanged, so performance-critical compilation phases are unaffected. The additional metadata storage and checking only occur when errors are being reported, which is already a relatively expensive operation.

During trait resolution, the only additional work is recording traceable metadata in obligation causes. This requires a small amount of additional memory to store a flag or enum variant indicating whether each obligation is traceable. Modern compilers already store extensive metadata in obligation structures for debugging and error reporting purposes, so adding one more bit of information has negligible impact on memory usage.

The error reporting phase requires traversing cause chains or proof trees to identify traceable obligations, but this traversal is comparable to the existing traversal that identifies leaf obligations. The implementation can cache traceable information computed during traversal to avoid redundant checks. The overall asymptotic complexity of error reporting remains unchanged.

For successful compilations where no errors occur, the performance impact is effectively zero. The traceable metadata exists in data structures but is never accessed because error reporting is never invoked. This ensures that the feature does not slow down successful builds, which represent the majority of compilation operations in typical development workflows.

There may be a small increase in compiler binary size due to the additional code for handling traceable metadata, but this increase is minimal compared to the overall compiler size. The implementation can leverage existing infrastructure for obligation metadata, minimizing new code. The trade-off between slightly larger compiler binaries and significantly better error messages is clearly favorable.

### 8.2 Error Message Verbosity Trade-offs

A key concern with any diagnostic improvement is avoiding making error messages too verbose. The `#[diagnostic::traceable]` attribute addresses this by giving control to library authors who can make informed decisions about which bounds are semantically significant. This prevents the error message explosion that would occur if the compiler naively reported all failed obligations.

For codebases that do not use traceable annotations, error message behavior remains exactly as before. This ensures backward compatibility and prevents any regression in diagnostic quality for existing code. Library authors can gradually adopt traceable annotations over time as they identify which bounds would benefit from explicit reporting.

When traceable annotations are used appropriately, error messages become more targeted rather than more verbose. The annotation directs the compiler to report the specific information that explains the problem, potentially replacing a vague high-level error with a clear low-level explanation. The notes that trace the dependency chain add context without overwhelming the user.

There is a risk of overuse where library authors mark too many bounds as traceable, producing errors that report numerous failures when a single root cause would be clearer. Documentation and usage guidelines should emphasize that traceable is for bounds that represent semantic requirements, not for every bound in every where clause. Code review and linter tools can help enforce appropriate usage.

In edge cases where multiple independent traceable bounds fail simultaneously, error messages will be longer than if only one error were reported. However, this reflects the reality that multiple problems exist and all need to be fixed. Identifying multiple issues in one compilation pass is preferable to the user fixing one issue, recompiling, and discovering the next issue iteratively.

### 8.3 Interaction with Future Language Features

The design of `#[diagnostic::traceable]` is forward-compatible with planned and potential future Rust language features. Trait specialization, if stabilized, would interact naturally with traceable annotations. More specific implementations could have different traceable bounds than general implementations, allowing fine-grained control over error messages based on which implementation is being considered.

The potential for negative trait bounds or exclusion constraints would complement traceable annotations. An implementation might mark positive bounds as traceable while leaving negative bounds unmarked, ensuring that error messages focus on missing capabilities rather than presence of exclusionary traits. This would improve error messages for complex generic constraints.

Associated type enhancements such as generic associated types already work with the current proposal since associated type bounds in where clauses can be marked traceable. Future associated type features would inherit this support automatically. If Rust gains the ability to have associated const constraints in where clauses, those could also be marked traceable following the same pattern.

Const generics and const evaluation in trait bounds interact well with traceable annotations. A bound requiring a const parameter to satisfy certain properties could be marked traceable, ensuring that errors about invalid const values are reported clearly. As Rust expands compile-time computation capabilities, traceable annotations help ensure that type-level and const-level computation errors remain comprehensible.

Potential future diagnostic attributes could compose with traceable. For example, a combined annotation that marks a bound as both traceable and carrying a custom error message would leverage both mechanisms. The compiler could support attribute combinations that provide fine-grained control over error reporting without introducing complex new syntax.

### 8.4 Migration Path for Existing Code

Existing code requires no changes when the `#[diagnostic::traceable]` attribute is introduced. The attribute is purely additive, affecting only error reporting and not changing language semantics. Code that compiles successfully before the attribute is added continues to compile successfully afterward. Code that fails to compile may receive improved error messages if dependencies are updated to use traceable annotations.

Library authors can adopt traceable annotations incrementally. A library can start by identifying the most problematic error message scenarios reported by users and adding traceable annotations to the specific blanket implementations involved. The library can be released with these annotations, and user feedback can guide further annotation additions. This gradual approach avoids requiring large upfront analysis.

Libraries can provide documentation explaining which bounds are marked traceable and why. This helps users understand the library's structure and expectations. Documentation can include examples of error messages that demonstrate how traceable bounds are reported, giving users confidence that they will receive clear diagnostics when using the library incorrectly.

For libraries that previously introduced workaround traits for diagnostic purposes, migration involves removing those traits and replacing them with traceable annotations on the actual semantic bounds. This simplifies the library API while maintaining or improving error message quality. A transition period might keep both mechanisms temporarily to ensure backward compatibility before removing the workaround traits.

The Rust ecosystem can develop linting tools that suggest where traceable annotations might be beneficial. A lint could identify blanket implementations with deep where clause chains and recommend considering traceable annotations. Community best practices can emerge around appropriate usage, with high-quality libraries serving as examples.

### 8.5 Documentation and User Guidance

Effective use of `#[diagnostic::traceable]` requires clear documentation explaining when and how to use the attribute. The Rust reference should include a section on diagnostic attributes that covers traceable alongside on_unimplemented and do_not_recommend, explaining what each attribute does and how they interact.

The Rust book could include examples of traceable in the chapters on traits and error handling, demonstrating how library authors can improve error messages for their users. The examples should emphasize the principle of marking semantically significant bounds, helping authors develop intuition about appropriate usage.

Library authors should be guided to think about their trait bounds from a user perspective. Bounds that encode fundamental requirements of the library's abstractions should be marked traceable. Bounds that represent implementation details or internal coordination between traits can remain unmarked. A useful heuristic is to mark bounds that, if unsatisfied, indicate that the user's type is missing a fundamental capability.

Documentation should also explain the propagation behavior: that marking a bound as traceable causes all nested obligations from that bound to be prioritized in error reporting. This helps authors understand that they do not need to mark every level of a delegation chain, only the semantically meaningful entry points.

Error message examples should be included showing the difference between traceable and non-traceable errors. Library documentation can include an "Error Messages" section showing common error scenarios and the exact error text users will see. This sets user expectations and demonstrates the library's commitment to diagnostic quality.

---

## Chapter 9: Alternatives Considered

### Chapter Outline

This chapter examines alternative approaches that were considered for improving error messages in deep trait bound scenarios. We analyze compiler flags for diagnostic verbosity, heuristic improvements without annotations, trait-level versus implementation-level annotations, automatic detection of deep chains, and explain why the proposed design is superior to these alternatives. Understanding the trade-offs helps justify the design choices and provides context for future discussions.

### 9.1 Compiler Flags for Diagnostic Verbosity

One alternative would be to add compiler flags that control error message verbosity, allowing users to request more detailed obligation information. A flag like `-Z verbose-trait-errors` could instruct the compiler to report all failed obligations without filtering. This approach has the advantage of not requiring any language changes and being available immediately to users who need more information.

However, compiler flags have significant drawbacks. They impose a uniform policy on all errors in a compilation, whereas different errors may need different levels of detail. A deeply nested CGP error might benefit from verbose reporting while a simple trait bound failure might become harder to understand with too much information. A flag cannot provide the fine-grained control necessary for optimal error messages.

Compiler flags also create a poor user experience. Users must discover that the flag exists, remember to use it when errors are confusing, and tolerate degraded error messages for all other errors in exchange for improved messages for the problematic ones. This adds friction to the development workflow and requires users to become compiler experts rather than focusing on their application logic.

From a library author perspective, flags do not allow communicating design intent. The library author knows which bounds are semantically significant but cannot express this knowledge in the source code. Users must manually enable verbose errors and filter through verbose output to identify relevant information. This places burden on users that should be handled by the library design.

Flags also create ecosystem fragmentation. Different projects might recommend different flag settings, tutorials might or might not mention flags, and users moving between projects would need to adjust their workflows. The Rust philosophy of sensible defaults and explicit annotations is better served by in-code attributes than by command-line configuration.

### 9.2 Heuristic Improvements Without Attribute

Another alternative is improving the compiler's filtering heuristics to better identify root causes automatically. Machine learning or pattern recognition could potentially detect when intermediate trait failures are less important than leaf failures. The compiler could recognize common patterns like blanket implementation chains and adjust reporting accordingly.

The fundamental limitation of this approach is that semantic significance cannot be reliably inferred from syntax alone. Two syntactically identical blanket implementations might have different semantic roles in different libraries. One might be an implementation detail while the other represents a fundamental abstraction. Without explicit information from the author, the compiler cannot distinguish these cases.

Heuristic improvements risk introducing subtle bugs where edge cases produce poor error messages. Different code patterns might trigger different heuristics, leading to inconsistent error reporting quality. Users would have difficulty understanding why some errors are clear while others remain confusing. The lack of explicit control makes debugging the diagnostics themselves challenging.

Pattern-based heuristics would need constant maintenance as new coding patterns emerge. As Rust libraries explore new ways to use the trait system, the heuristics would require updates to handle new patterns. This creates an ongoing maintenance burden for the compiler team and a lag between pattern adoption and good error message support.

The proposed attribute combines the benefits of good heuristics with explicit authorship. The compiler can use heuristics for unannotated code while respecting explicit annotations where authors have provided them. This hybrid approach provides better defaults while allowing fine-grained control where needed.

### 9.3 Trait-Level Annotations vs Implementation-Level Annotations

The proposed design places `#[diagnostic::traceable]` on implementation where clauses rather than on trait definitions. An alternative would be to allow marking traits themselves as requiring traceable reporting. This would avoid requiring annotations on every implementation of a trait and would provide a single point of control.

Trait-level annotations are problematic because different implementations of a trait might have different diagnostic needs. A trait with both a simple explicit implementation and a complex blanket implementation should not impose the same diagnostic policy on both. The explicit implementation might not benefit from traceable reporting while the blanket implementation critically depends on it.

Trait-level annotations also reduce flexibility for downstream users. If an upstream trait is marked as always requiring traceable reporting, downstream libraries that use that trait would be locked into this behavior. Implementation-level annotations allow each library to make its own decisions about which constraints in its implementations are semantically significant.

The implementation-level approach better respects the principle of locality. When reading an implementation's where clause, annotations on the bounds make it immediately clear which constraints are considered important. A reader does not need to look up trait definitions to understand the diagnostic policy. Code review can assess whether the annotations are appropriate in context.

However, there may be value in allowing both trait-level and implementation-level annotations in the future. A trait could have default traceable annotations that implementations inherit but can override. This would provide convenience for traits that always have the same diagnostic needs while preserving flexibility. The current proposal starts with implementation-level annotations as the foundation.

### 9.4 Automatic Detection of Deep Chains

The compiler could potentially detect deep delegation chains automatically and adjust error reporting without explicit annotations. If a trait bound requires checking more than some threshold number of nested obligations, the compiler could automatically report leaf failures rather than intermediate ones. This would require no language changes and would help all code automatically.

The challenge with automatic detection is determining appropriate thresholds. A fixed depth threshold would be too conservative for some code and too aggressive for others. Code with shallow but semantically significant chains would not benefit, while code with deep but intentional abstraction layers would report excessive detail.

Automatic detection also conflates depth with diagnostic significance. A deep chain might represent carefully layered abstractions where reporting intermediate levels is actually more helpful than reporting leaves. Conversely, a shallow chain might have a leaf failure that is extremely important to surface. Depth is a proxy for significance but not a reliable one.

The implementation complexity of automatic detection would be substantial. The compiler would need to analyze chains during error reporting, compute appropriate thresholds dynamically based on code structure, and handle edge cases where chains have varying depths through different paths. This complexity introduces risk of bugs and unexpected behavior.

Explicit annotations provide clarity and predictability. Library authors mark what they consider significant, and the compiler reliably honors those marks. Users can trust that annotated bounds will be reported clearly without needing to understand compiler heuristics. The explicitness trades a small annotation burden for substantial gains in reliability.

### 9.5 Why the Proposed Design is Superior

The `#[diagnostic::traceable]` attribute design superior to alternatives because it balances control, simplicity, and forward compatibility. Library authors gain fine-grained control over error reporting without requiring complex language features. The attribute syntax is simple and follows established patterns for diagnostic annotations. The implementation can start minimal and be enhanced over time without breaking changes.

The design respects Rust's philosophy of explicit is better than implicit. Rather than relying on heuristics that users must learn and predict, the feature provides an explicit annotation whose meaning is clear from its name and placement. This reduces cognitive load and makes code more maintainable.

The attribute integrates naturally with existing diagnostic infrastructure. It complements rather than replaces existing attributes like on_unimplemented, and it leverages existing obligation tracking mechanisms. The implementation requires minimal new compiler infrastructure, reducing risk and development time.

The design is conservative in scope, starting with implementation where clauses and allowing future expansion. This phased approach allows gaining implementation experience and gathering feedback before expanding to other contexts. The initial scope is sufficient to address the most critical error message problems while leaving room for refinement.

Most importantly, the proposed design actually solves the problem for real users. CGP code with traceable annotations produces clear error messages that identify root causes. Other patterns with deep trait bound chains gain the same benefit. The attribute provides practical value immediately while building toward a robust long-term solution.

---

## Chapter 10: Unresolved Questions and Future Work

### Chapter Outline

This final chapter identifies remaining questions and potential future enhancements for `#[diagnostic::traceable]`. We discuss interaction with specialization when it becomes available, handling of negative trait bounds, extensions to associated item bounds, integration with IDE tooling for quick fixes, and opportunities to collect telemetry on attribute usage. These items do not block the initial implementation but provide direction for future improvements.

### 10.1 Interaction with Specialization

When specialization is stabilized in Rust, implementations will be able to overlap with more specific implementations taking precedence over more general ones. The interaction between specialization and traceable annotations needs careful consideration. If a general blanket implementation and a specialized implementation both apply to a type, and they have different traceable annotations, which annotations should be honored?

The most consistent behavior is that the selected implementation's traceable annotations are what matter. If the compiler selects the specialized implementation, its traceable bounds are checked and reported according to its annotations, ignoring the general implementation's annotations. This respects the principle that specialization selects a single implementation to use.

However, there may be value in reporting constraint failures from multiple candidate implementations when specialization is ambiguous. If the compiler cannot determine which implementation should be selected, reporting traceable bounds from all viable candidates could help the user understand what is needed to resolve the ambiguity. This would require more sophisticated error reporting that explains specialization conflicts.

Another question is whether traceable annotations should affect specialization priority. Currently, annotations do not influence implementation selection. Should an implementation with more traceable bounds be considered a better match than one with fewer traceable bounds? This seems unlikely to be helpful and could introduce surprising behavior, so the answer is probably no.

The interaction with negative specialization bounds is also unclear. If a specialized implementation has negative bounds that exclude certain types and positive bounds that are traceable, how should errors be reported when both types of bounds are relevant? The traceable annotation should apply only to positive bounds, leaving negative bound reporting to follow general rules.

### 10.2 Negative Trait Bounds and Exclusion Constraints

Rust currently does not have syntax for negative trait bounds that explicitly require a type not to implement a trait. If negative bounds are added to the language, their interaction with traceable needs specification. Should negative bounds be allowed to have traceable annotations, and if so, what would that mean?

A traceable negative bound would indicate that when the negative constraint is violated (meaning the type does implement the excluded trait), this should be reported clearly. This could be useful for bounds that express mutual exclusion between traits, where implementing both traits is a semantic error that users should understand clearly.

However, negative bounds are often used for implementation selection rather than expressing semantic requirements. A blanket implementation might use negative bounds to avoid overlapping with other implementations, making the negative bound an implementation detail. Marking such bounds as traceable could produce confusing errors about unexpected trait implementations.

The resolution might be to allow traceable on negative bounds but document that it should be used sparingly, only when violation of the negative constraint represents a genuine semantic error. Linting tools could warn about traceable negative bounds in contexts where they seem likely to be implementation details.

### 10.3 Diagnostic Annotations for Associated Types

The current proposal does not support traceable annotations on associated type bounds, but there may be value in extending support to this context. Associated types often have where clauses that constrain the associated type to implement certain traits. When these constraints are unsatisfied, error reporting could benefit from traceable annotations.

The challenge is that associated type constraints manifest at different points than trait implementation constraints. An associated type constraint might fail when the associated type is normalized, when it is used in a function signature, or when it appears in another trait bound. The causal chain is more complex than for simple trait bounds.

A design for traceable associated type bounds would need to specify when and how the annotations affect error reporting. If a trait has an associated type with a traceable bound, should this affect reporting whenever that trait is used, or only when the associated type specifically is accessed? The semantics need careful definition.

One approach would be to treat traceable associated type bounds as affecting only errors that directly involve the associated type. If code attempts to use the associated type in a context requiring the bound, and the bound is not satisfied, the traceable annotation ensures this is reported clearly. This scoping keeps the semantics manageable.

### 10.4 Integration with IDE Quick Fixes

IDE integrations for Rust provide quick fix suggestions that help users correct errors. When a trait bound is not satisfied, the IDE might suggest adding a derived implementation, implementing the trait manually, or changing the type. Integration between traceable annotations and IDE quick fixes could enhance the development experience.

An IDE could recognize when an error involves a traceable bound and prioritize suggesting fixes for that bound. If a traceable error indicates that a type is missing a particular field, the quick fix could offer to add that field. If a traceable bound requires implementing a trait, the quick fix could scaffold the trait implementation.

The challenge is communicating enough context from the compiler to the IDE. The error message must not only identify the traceable bound failure but also provide structured information about what kind of fix might be appropriate. This might involve extending the error reporting system to emit machine-readable suggestions alongside human-readable messages.

Traceable bounds marked with custom error messages via on_unimplemented could include structured suggestions that IDEs parse. The combination of traceable for visibility and on_unimplemented for suggestions would provide both clear errors and actionable fixes. This integration requires coordination between compiler error reporting and IDE tool protocols.

### 10.5 Telemetry and Usage Patterns

Once `#[diagnostic::traceable]` is deployed, collecting telemetry on its usage patterns would inform future improvements. Anonymized data about how often traceable annotations appear in code, what kinds of bounds are typically marked, and whether users encounter errors involving traceable bounds could guide evolution of the feature.

Understanding which patterns benefit most from traceable annotations helps prioritize documentation and example improvements. If certain trait patterns consistently use traceable while others rarely do, this reveals which abstractions face the most diagnostic challenges. The community can develop best practices based on empirical usage data.

Telemetry could also identify misuse patterns. If many codebases mark nearly all bounds as traceable, this suggests confusion about the feature's purpose. Conversely, if few codebases use the feature despite having deep trait bound chains, this suggests need for better documentation or discovery. Usage data helps assess whether the feature is succeeding at its goals.

Error message quality could be assessed through user feedback coupled with telemetry. If users report that errors involving traceable bounds are particularly clear or particularly confusing, this feedback guides error message formatting improvements. A/B testing different error message presentations for traceable bounds could optimize message clarity.

The implementation could include debug logging that records traceable obligation processing during development. This logging would help compiler developers understand how traceable interacts with trait resolution and identify opportunities for optimization or enhancement. Debug logs separate from user-facing telemetry provide implementation insights.

---

This RFC proposes `#[diagnostic::traceable]` as a practical solution to a real problem affecting Rust users working with advanced trait patterns. The attribute empowers library authors to mark semantically significant trait bounds, ensuring that errors involving these bounds are reported clearly even in the presence of deep delegation chains. The implementation leverages existing compiler infrastructure, the design is forward-compatible, and the benefits extend beyond CGP to other trait-heavy patterns. By making diagnostic significance explicit rather than inferred, the attribute improves error message quality while respecting Rust's principles of clarity and predictability.