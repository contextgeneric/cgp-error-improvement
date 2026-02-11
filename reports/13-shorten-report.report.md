## Chapter 1: Problem Statement

### 1.1 Context-Generic Programming Overview

Context-Generic Programming (CGP) is a Rust design pattern that achieves modularity through systematic use of blanket trait implementations and type-level delegation. The pattern separates traits into **consumer traits** (the interface users call) and **provider traits** (the interface implementations fulfill), with automatic delegation from consumer to provider based on type-level configuration.

**Core mechanisms:**

- **Provider traits**: Implementation interfaces parameterized over context types
- **DelegateComponent**: Type-level table mapping components to providers  
- **HasField**: Dependency injection via compile-time field access
- **Blanket implementations**: Consumer traits automatically forward to configured providers

For example, a `CanCalculateArea` consumer trait delegates to an `AreaCalculator` provider trait. The context configures which provider to use via `delegate_components!`, and the provider declares dependencies like `Context: HasRectangleFields`. This creates **delegation chains** where satisfying one trait obligation creates new obligations, often 5+ levels deep.

CGP's modularity benefits come at a cost: when dependencies aren't satisfied (like a missing struct field), the error messages become incomprehensible.

### 1.2 The Error Message Problem

CGP's deep delegation chains expose a fundamental mismatch between how the Rust compiler reports errors and what users need to diagnose problems. The compiler's error filtering heuristics assume shallow dependency chains typical in conventional Rust code, but CGP deliberately constructs deep hierarchies where a single missing field cascades through multiple layers.

**Example: Missing `height` field**

```rust
#[derive(HasField)]
pub struct Rectangle {
    pub width: f64,
    // pub height: f64,  // Commented out to trigger error
}

delegate_components! {
    Rectangle {
        AreaCalculatorComponent: RectangleArea,
    }
}

check_components! {
    CanUseRectangle for Rectangle {
        AreaCalculatorComponent,
    }
}
```

**What the compiler reports (abbreviated):**

```
error[E0277]: the trait bound `Rectangle: CanUseComponent<...>` is not satisfied
  --> src/base_area.rs:41:9
   |
41 |         AreaCalculatorComponent,
   |         ^^^^^^^^^^^^^^^^^^^^^^^ unsatisfied trait bound
   |
   = help: the trait `HasField<Symbol<6, Chars<'h', Chars<'e', ...>>>>>` 
           is not implemented for `Rectangle`
note: required for `Rectangle` to implement `HasRectangleFields`
note: required for `RectangleArea` to implement `IsProviderFor<...>`
   = note: required for `Rectangle` to implement `CanUseComponent<...>`
```

**Problems with this output:**

1. **Root cause hidden**: The missing `height` field is buried in cryptic `Symbol` syntax
2. **Excessive verbosity**: Multiple "required for" notes that all essentially say the same thing
3. **Wrong entry point**: Error points to component declaration, not the struct definition
4. **No actionable guidance**: Doesn't tell user to add the field
5. **Truncated information**: Character encoding shows as `Chars<'h', Chars<'e', ...>` hiding the field name

**Why this happens:**

The compiler processes each trait obligation independently. When checking `CanUseComponent`, it checks `IsProviderFor`, which checks provider bounds, which checks `HasRectangleFields`, which checks `HasField` for specific fields. Each failed check generates an error. The compiler's deduplication logic keeps some errors but discards others based on heuristics designed for shallow chains. For CGP's deep chains, this removes crucial information while keeping redundant context.

### 1.3 What Users Actually Need

Effective CGP error messages should:

**1. Identify the root cause clearly**
- "Missing field `height` in struct `Rectangle`"
- Not: cryptic Symbol types or truncated character sequences

**2. Use CGP-aware terminology**
- Distinguish components, providers, consumer traits, and check traits
- Explain that `CanUseRectangle` is a *check trait* verifying component availability
- Not: generic "trait bound not satisfied" language

**3. Show relevant delegation chain**
- Condensed view: "Component `AreaCalculator` → provider `RectangleArea` → requires `HasRectangleFields` → needs field `height`"
- Not: every intermediate trait resolution step

**4. Provide actionable fixes**
- "Add `pub height: f64` to the `Rectangle` struct"
- "Add `#[derive(HasField)]` if the struct is missing it"
- Not: generic suggestions to "implement the trait"

**5. Handle higher-order providers gracefully**
- For `ScaledArea<RectangleArea>`, indicate the error in the outer provider is caused by the inner provider
- Not: duplicate errors for both providers

**Example of desired output:**

```
error: missing field `height` required by CGP component

The struct `Rectangle` is missing the required field `height`.
This field is required by component `AreaCalculatorComponent`.

Delegation chain:
  → Provider `RectangleArea` requires `HasRectangleFields`
  → `HasRectangleFields` requires field `height: f64`

To fix:
  - Add `pub height: f64` to `Rectangle` at line 26
```

### 1.4 Why an External Tool

**Rapid iteration**: Modifying the Rust compiler requires navigating the RFC process, implementing changes in a large complex codebase, and waiting for releases. An external tool can iterate quickly based on user feedback.

**CGP-specific heuristics**: The compiler must serve all Rust users and cannot encode patterns specific to one programming methodology. An external tool can make assumptions that would be inappropriate in the compiler.

**Sufficient information available**: The compiler's JSON diagnostic output (via `--message-format=json`) includes:
- Structured diagnostic tree with error code, message, spans
- Child diagnostics with notes explaining "required for" relationships  
- Rendered text that sometimes contains information not in structured fields
- Complete source location information

While the compiler filters this information heavily before displaying it to users, the raw JSON contains enough detail to reconstruct dependency chains and identify root causes.

**Proof of concept value**: A successful external tool demonstrates that better error messages are achievable, potentially motivating compiler improvements. Even if the compiler eventually adopts similar logic, the external tool provides immediate value to current CGP users.

**Implementation approach**: The tool intercepts `cargo check` compilation, parses JSON diagnostics, applies CGP-specific analysis to identify root causes and eliminate redundancy, then renders improved messages using libraries like miette for rich terminal output.

---

## Chapter 2 Revision Plan: Input Processing

### Objectives
Merge original Chapters 3-4 into a single chapter covering:
1. JSON diagnostic format structure
2. Key fields cargo-cgp extracts
3. CGP-specific pattern matching

### Structure
- **2.1 JSON Diagnostic Format** (from Ch 3.1-3.8)
  - Message types: CompilerMessage is what we care about
  - Diagnostic structure: message, level, code, spans, children
  - Rendered vs structured fields
  - Skip: extensive serialization details, forward compatibility concerns

- **2.2 Extracting CGP Patterns** (from Ch 3.9-3.11 + Ch 6)
  - HasField patterns in help messages
  - IsProviderFor in notes
  - Component names and delegation chains
  - Field name extraction from Symbol types
  - Skip: exhaustive pattern catalog, detailed parsing algorithms

- **2.3 Using cargo_metadata** (from Ch 4, heavily condensed)
  - Why use it: type-safe parsing, forward compatibility
  - Key types: Message, Diagnostic, DiagnosticSpan
  - Skip: library internals, MessageIter details, extensive API documentation

### Content to Cut
- Chapter 3 sections on macro expansion tracking (3.7)
- Detailed forward compatibility discussion (3.12)
- Most of Chapter 4 (library documentation that belongs in code comments)
- Extensive examples of JSON structures (show key examples only)

### Content to Preserve
- Concrete JSON examples showing CGP errors
- Pattern matching rules for CGP constructs
- List of fields cargo-cgp needs to extract
- Symbol type decoding logic

### Target Length
Reduce from ~15-20 pages to ~5-6 pages

---

## Chapter 2: Input Processing

### 2.1 JSON Diagnostic Format

Cargo's `--message-format=json` flag outputs newline-delimited JSON objects representing compilation events. Each line is a `Message` enum variant. For cargo-cgp, we care about:

**Message::CompilerMessage** - Contains a diagnostic from rustc:
```json
{
  "reason": "compiler-message",
  "package_id": "...",
  "target": {...},
  "message": {
    "message": "the trait bound `Rectangle: CanUseComponent<...>` is not satisfied",
    "code": {"code": "E0277", "explanation": "..."},
    "level": "error",
    "spans": [
      {
        "file_name": "src/base_area.rs",
        "line_start": 41,
        "column_start": 9,
        "is_primary": true,
        "label": "unsatisfied trait bound",
        "text": [{"text": "        AreaCalculatorComponent,"}]
      }
    ],
    "children": [
      {
        "message": "the trait `HasField<Symbol<...>>` is not implemented...",
        "level": "help",
        "spans": [...]
      },
      {
        "message": "required for `Rectangle` to implement `HasRectangleFields`",
        "level": "note",
        "spans": []
      }
    ],
    "rendered": "error[E0277]: the trait bound..."
  }
}
```

**Key fields:**

- **message**: Human-readable error text (often truncated)
- **code**: Error code (E0277 for trait bounds)
- **spans**: Source locations with labels
  - `is_primary`: true for the main error location
  - `label`: Short description shown at the error location
  - `text`: Actual source code lines
- **children**: Nested diagnostics (help, note, warning)
  - Help messages often contain trait implementation details
  - Note messages contain "required for" dependency chains
- **rendered**: Pre-formatted text output (may contain info not in structured fields)

**Other message types** (we mostly ignore these):
- **Message::CompilerArtifact**: Successful compilation of a crate
- **Message::BuildScriptExecuted**: Build script output
- **Message::BuildFinished**: Final build status

### 2.2 Extracting CGP Patterns

CGP errors have distinctive patterns we can recognize and extract:

#### Pattern 1: Missing Field (HasField)

**In help messages:**
```
the trait `HasField<Symbol<6, Chars<'h', Chars<'e', Chars<'i', ...>>>>>` 
is not implemented for `Rectangle`
```

**Extract:**
- Field name from `Symbol<N, Chars<...>>` pattern
- Expected length N (e.g., 6 for "height")
- Character sequence: `Chars<'h', Chars<'e', Chars<'i', Chars<'g', Chars<'h', Chars<'t', Nil>>>>>>`
- Target type: `Rectangle`

**Challenges:**
- Characters may be truncated: `Chars<'h', ...>` hides the full name
- Some characters shown as `Chars<_,` (no quotes) indicating compiler redaction
- Use Unicode replacement character � for hidden characters
- Compare extracted length with expected length to detect incompleteness

**Detection logic:**
```rust
if message.contains("HasField<Symbol<") {
    let length = extract_between("Symbol<", ",");
    let chars = extract_chars_sequence();  // Build from Chars<'x', ...> pattern
    let target_type = extract_after("is not implemented for `");
    // Mark as possibly incomplete if chars.len() < length
}
```

#### Pattern 2: Provider Requirements (IsProviderFor)

**In note messages:**
```
required for `RectangleArea` to implement 
`IsProviderFor<AreaCalculatorComponent, Rectangle>`
```

**Extract:**
- Provider type: `RectangleArea`
- Component: `AreaCalculatorComponent`
- Context: `Rectangle`

**Pattern in messages:**
- "required for `{Provider}` to implement `IsProviderFor<{Component}, {Context}>`"

#### Pattern 3: Component Names

**Look for:**
- Types ending in "Component" (e.g., `AreaCalculatorComponent`)
- `CanUseComponent<XyzComponent>` patterns
- Component names in delegation notes

**Derive provider trait name:**
- Strip "Component" suffix: `AreaCalculatorComponent` → `AreaCalculator`

#### Pattern 4: Delegation Chains

**In child diagnostics (level: "note"):**
```
required for `Rectangle` to implement `HasRectangleFields`
required for `RectangleArea` to implement `IsProviderFor<...>`
required for `Rectangle` to implement `CanUseComponent<...>`
```

**Extract sequence:**
- Parse each "required for `{Type}` to implement `{Trait}`" note
- Build dependency list showing what requires what
- Detect cycles and redundancy

#### Pattern 5: Check Traits

**In note messages:**
```
required by a bound in `CanUseRectangle`
```

**Extract:**
- Check trait name from "required by a bound in `{TraitName}`"
- Note: This is the check trait generated by `check_components!`, not the consumer trait

#### Pattern 6: Higher-Order Providers

**Detect nested providers:**
```
ScaledArea<RectangleArea>
```

**Identify inner providers:**
- Parse generic arguments in provider types
- Look for provider names inside angle brackets
- Mark inner provider as potential root cause when outer provider fails

### 2.3 Using cargo_metadata

The `cargo_metadata` crate provides strongly-typed parsing of Cargo's JSON output:

```rust
use cargo_metadata::Message;

for message in Message::parse_stream(reader) {
    match message? {
        Message::CompilerMessage(msg) => {
            if is_cgp_diagnostic(&msg.message) {
                process_cgp_error(msg);
            } else {
                // Pass through non-CGP errors unchanged
                print_original(msg);
            }
        }
        Message::CompilerArtifact(artifact) => {
            eprintln!("  Compiling {}", artifact.target.name);
        }
        _ => {} // Ignore other message types
    }
}
```

**Why use cargo_metadata:**
- **Type safety**: Structured access to fields vs manual JSON parsing
- **Forward compatibility**: Library maintainers handle format changes
- **Complete types**: All diagnostic fields available through safe APIs
- **Streaming**: Process messages as they arrive without buffering entire output

**Key types:**

```rust
pub struct CompilerMessage {
    pub message: Diagnostic,
    pub package_id: PackageId,
    pub target: Target,
}

pub struct Diagnostic {
    pub message: String,
    pub code: Option<DiagnosticCode>,
    pub level: DiagnosticLevel,  // Error, Warning, Help, Note
    pub spans: Vec<DiagnosticSpan>,
    pub children: Vec<Diagnostic>,  // Nested diagnostics
    pub rendered: Option<String>,
}

pub struct DiagnosticSpan {
    pub file_name: String,
    pub line_start: usize,
    pub column_start: usize,
    pub line_end: usize,
    pub column_end: usize,
    pub is_primary: bool,
    pub label: Option<String>,
    pub text: Vec<DiagnosticSpanLine>,
}
```

**Identifying CGP diagnostics:**

```rust
fn is_cgp_diagnostic(diag: &Diagnostic) -> bool {
    let cgp_patterns = [
        "CanUseComponent",
        "IsProviderFor", 
        "HasField",
        "cgp_component",
        "delegate_components",
    ];
    
    cgp_patterns.iter().any(|p| 
        diag.message.contains(p) || 
        diag.children.iter().any(|c| c.message.contains(p))
    )
}
```

---

## Chapter 3: Architecture

### 3.1 Data Flow Overview

```
┌─────────────────┐
│  cargo check    │
│  --message-     │
│   format=json   │
└────────┬────────┘
         │ JSON stream
         ▼
┌─────────────────┐
│  Parser         │  Recognize CGP patterns
│                 │  Extract field names, providers, etc.
└────────┬────────┘
         │ CompilerMessage + extracted patterns
         ▼
┌─────────────────┐
│  DiagnosticDB   │  Merge related errors
│                 │  Group by location + component
└────────┬────────┘
         │ DiagnosticEntry (merged)
         ▼
┌─────────────────┐
│  Analyzer       │  Build dependency graphs
│                 │  Identify root causes
│                 │  Deduplicate provider chains
└────────┬────────┘
         │ DiagnosticEntry (analyzed)
         ▼
┌─────────────────┐
│  Formatter      │  Generate improved messages
│                 │  Render with miette
└────────┬────────┘
         │ Formatted text
         ▼
┌─────────────────┐
│  Terminal       │
└─────────────────┘
```

**Processing stages:**

1. **Parse**: Stream JSON from cargo, identify CGP vs non-CGP errors
2. **Collect**: Accumulate CGP diagnostics in database
3. **Analyze**: Process collected diagnostics to find root causes
4. **Render**: Generate and display improved error messages

Non-CGP errors pass through unchanged to preserve normal Rust error output.

### 3.2 Core Components

#### Parser (cgp_patterns.rs)

**Responsibility:** Pattern matching on diagnostic messages to extract CGP-specific information.

**Key functions:**
- `is_cgp_diagnostic(diag)` - Detect CGP-related errors
- `extract_field_info(diag)` - Parse HasField patterns for missing fields
- `extract_provider_relationship(msg)` - Parse IsProviderFor constraints  
- `extract_component_info(msg)` - Find component names
- `extract_check_trait(msg)` - Identify check traits

**Pattern matching approach:**
- String search for CGP keywords
- Regex-like parsing for structured patterns (Symbol, Chars)
- Heuristic inference when information is incomplete

#### DiagnosticDB (diagnostic_db.rs)

**Responsibility:** Collect and merge related diagnostics that describe the same underlying error.

**Why merge:** The compiler may emit multiple diagnostics for the same problem (e.g., once when checking the provider trait, again when checking CanUseComponent). Merging prevents duplicate errors in output.

**Merging strategy:**
- **Key**: `(source_location, component_type)`
- **Behavior**: If a new diagnostic matches an existing key, merge information rather than creating separate entry
- **Merged info**: Combine delegation notes, provider relationships, field info

**Key operations:**
- `add_diagnostic(msg)` - Add or merge a compiler message
- `get_active_entries()` - Retrieve non-suppressed entries
- `render_cgp_diagnostics()` - Convert entries to formatted output

#### Analyzer (root_cause.rs)

**Responsibility:** Process merged diagnostics to identify root causes and filter redundancy.

**Root cause identification:**
- **Primary indicator**: Presence of `field_info` (missing field is typically root cause)
- **Secondary indicator**: Provider relationships with unsatisfied bounds

**Deduplication:**
- **Provider nesting**: If `ScaledArea<RectangleArea>` and `RectangleArea` both fail with same component, keep only outer provider
- **Delegation chain**: Remove redundant "required for" notes that don't add information
- **Pattern**: Detect when inner provider type appears as generic argument in outer provider

**Key functions:**
- `deduplicate_provider_relationships(rels)` - Remove nested provider redundancy
- `deduplicate_delegation_notes(notes)` - Remove duplicate chain entries
- `detect_inner_providers(rels)` - Find providers nested inside others

#### Formatter (error_formatting.rs + render.rs)

**Responsibility:** Transform analyzed diagnostics into improved error messages.

**Message structure:**
1. **Summary**: "missing field `X` required by CGP component"
2. **Explanation**: What's wrong and why
3. **Context**: Component and check trait information
4. **Delegation chain**: Condensed dependency path
5. **Fix suggestions**: Actionable steps

**Rendering:**
- Use `miette` crate for rich terminal output
- Source code spans with labels
- Color coding (errors in red, suggestions in blue)
- Plain text fallback for non-terminal output

**Key functions:**
- `format_error_message(entry)` - Build CgpDiagnostic from DiagnosticEntry
- `format_delegation_chain(entry)` - Simplify and format dependency chain
- `render_diagnostic_plain(diag)` - Convert to terminal output

### 3.3 Key Data Structures

#### DiagnosticEntry

Represents a merged diagnostic with all extracted CGP information:

```rust
pub struct DiagnosticEntry {
    // Original compiler diagnostic
    pub original: Diagnostic,
    
    // Source location
    pub primary_span: Option<DiagnosticSpan>,
    pub error_code: Option<String>,  // e.g., "E0277"
    
    // Extracted CGP information
    pub field_info: Option<FieldInfo>,
    pub component_info: Option<ComponentInfo>,
    pub check_trait: Option<String>,
    pub provider_relationships: Vec<ProviderRelationship>,
    pub delegation_notes: Vec<String>,
    
    // Analysis results
    pub is_root_cause: bool,
    pub suppressed: bool,  // True if redundant
    pub has_other_hasfield_impls: bool,  // Affects fix suggestion
}
```

#### FieldInfo

Information about a missing field:

```rust
pub struct FieldInfo {
    pub field_name: String,           // e.g., "height"
    pub is_complete: bool,            // False if truncated
    pub has_unknown_chars: bool,      // True if contains �
    pub target_type: String,          // e.g., "Rectangle"
}
```

#### ProviderRelationship

Represents a provider → component → context dependency:

```rust
pub struct ProviderRelationship {
    pub provider_type: String,   // e.g., "RectangleArea"
    pub component: String,       // e.g., "AreaCalculatorComponent"
    pub context: String,         // e.g., "Rectangle"
}
```

#### ComponentInfo

Extracted component information:

```rust
pub struct ComponentInfo {
    pub component_type: String,     // e.g., "AreaCalculatorComponent"
    pub provider_trait: Option<String>,  // e.g., "AreaCalculator"
}
```

---

## Chapter 4: Analysis Pipeline

### 4.1 Extracting Dependencies

The compiler's JSON diagnostics scatter dependency information across multiple fields. We must reconstruct the full dependency chain from fragments.

#### From Child Diagnostics (Structured)

Child diagnostics with `level: "note"` contain "required for" relationships:

```json
{
  "level": "note",
  "message": "required for `Rectangle` to implement `HasRectangleFields`",
  "spans": []
}
```

**Pattern:** `"required for `{Type}` to implement `{Trait}`"`

**Extract:**
- Dependent type: `Rectangle`
- Required trait: `HasRectangleFields`

Store each as a delegation chain link. Order matters—notes appear in dependency order.

#### From IsProviderFor Notes

Provider relationships appear in notes:

```
required for `RectangleArea` to implement 
`IsProviderFor<AreaCalculatorComponent, Rectangle>`
```

**Extract three components:**
- Provider: `RectangleArea`
- Component: `AreaCalculatorComponent`
- Context: `Rectangle`

**Create ProviderRelationship:**
```rust
ProviderRelationship {
    provider_type: "RectangleArea",
    component: "AreaCalculatorComponent",
    context: "Rectangle",
}
```

#### From Rendered Text (Semi-Structured)

Sometimes information only appears in rendered text:

```
= help: the trait `HasField<Symbol<6, ...>>` is not implemented for `Rectangle`
        but trait `HasField<Symbol<5, ...>>` is implemented for it
```

The "but trait ... is implemented" tells us other fields exist, affecting fix suggestions (don't suggest adding `#[derive(HasField)]` if it's already there).

#### Building the Delegation Chain

Collect all "required for" notes in order:

```rust
let mut delegation_notes = Vec::new();
for child in diagnostic.children {
    if child.level == Note && child.message.contains("required for") {
        delegation_notes.push(child.message.clone());
    }
}
```

This preserves the chain showing how one requirement leads to another:
1. Required for `Rectangle` to implement `HasRectangleFields`
2. Required for `RectangleArea` to implement `IsProviderFor<AreaCalculatorComponent, Rectangle>`
3. Required for `Rectangle` to implement `CanUseComponent<AreaCalculatorComponent>`

### 4.2 Identifying Root Causes

**Goal:** Distinguish the actual problem (missing field) from cascade effects (failed provider traits).

#### Primary Indicator: Missing Fields

If a diagnostic has `field_info`, it's describing a missing field error. This is almost always the root cause:

```rust
if entry.field_info.is_some() {
    entry.is_root_cause = true;
}
```

**Why fields are root causes:**
- Concrete, actionable problem (add the field)
- Leaf node in dependency graph (nothing depends on it existing)
- Creates cascade of trait failures above it

#### Secondary Indicator: Provider Bounds

If no field_info but provider_relationships exist, check if provider has unsatisfied bounds:

```rust
if !entry.provider_relationships.is_empty() {
    // Provider trait not implemented due to some missing bound
    // Less clear-cut as root cause, but worth reporting
}
```

#### Ranking Strategy

When multiple candidates exist:

1. **Field errors** - Highest priority
2. **Provider bound errors** - Medium priority (but check for nesting—see deduplication)
3. **Generic CanUseComponent errors** - Lowest priority (usually transitive)

#### Identifying Entry Point

The primary_span should point to where the error manifests, but for CGP errors this is often the check_components! declaration. For better UX, we could try to identify:
- The struct definition (for missing field errors)
- The delegate_components! invocation (for misconfigured providers)

This requires additional span analysis beyond what we currently do.

### 4.3 Deduplication Strategy

Multiple diagnostics often describe the same underlying problem. Deduplication prevents confusing duplicate errors.

#### Provider Nesting

**Problem:** Errors from nested providers create redundancy.

**Example:**
```
1. ScaledArea<RectangleArea> can't implement AreaCalculator (missing field height)
2. RectangleArea can't implement AreaCalculator (missing field height)
```

Both errors describe the same missing field, but inner provider `RectangleArea` is the actual source.

**Detection:**
```rust
fn is_contained_type_parameter(inner: &str, outer: &str) -> bool {
    outer.contains(&format!("<{}>", inner)) ||
    outer.contains(&format!("<{},", inner)) ||
    outer.contains(&format!(", {}>", inner))
}
```

**Deduplication rule:**
If two `ProviderRelationship` entries have:
- Same component
- Same context
- One provider contains the other as a type parameter

→ Keep only the outer provider, suppress the inner

**Why keep outer:** Users configure the outer provider in delegate_components!. Mentioning the inner provider is an implementation detail.

**Add explanatory note:**
```
The error in `ScaledArea<RectangleArea>` is caused by 
the inner provider `RectangleArea`
```

#### Delegation Chain Simplification

**Problem:** Multiple notes may repeat the same information.

**Example:**
```
required for `Rectangle` to implement `HasRectangleFields`
required for `Rectangle` to implement `HasRectangleFields`  // duplicate
required for `RectangleArea` to implement `IsProviderFor<...>`
```

**Deduplication rule:**
```rust
fn deduplicate_delegation_notes(notes: &[String]) -> Vec<String> {
    let mut seen = HashSet::new();
    notes.iter()
        .filter(|n| seen.insert(n.clone()))
        .cloned()
        .collect()
}
```

**Additional simplification:**
- Remove module prefixes: `cgp::prelude::HasField` → `HasField`
- Replace internal trait names with user-friendly terms
- Truncate overly long type names with ellipsis

#### Location-Based Merging

**Problem:** The same error may be reported at multiple points in the trait checking process.

**Merging key:** `(file, line, column, component)`

**Strategy:**
```rust
if existing_entry.location == new_entry.location &&
   existing_entry.component == new_entry.component {
    // Merge new information into existing entry
    merge_field_info(existing, new);
    merge_provider_relationships(existing, new);
    merge_delegation_notes(existing, new);
} else {
    // Create new entry
}
```

**Merged information:**
- Union of delegation notes
- Union of provider relationships
- Take first non-None field_info

#### Suppression vs Deletion

Rather than deleting redundant entries, mark them as suppressed:

```rust
entry.suppressed = true;
```

**Benefits:**
- Debugging: Can inspect all diagnostics if needed
- Confidence: Know deduplication is working by checking suppressed count
- Future: Might add verbosity flag to show suppressed entries

**Rendering:**
```rust
let active_entries = db.entries
    .values()
    .filter(|e| !e.suppressed)
    .collect();
```

---

## Chapter 5: Output Generation

### 5.1 Design Principles

**1. Root Cause First**

Lead with the actionable problem, not the cascade:
- ✗ "the trait bound `Rectangle: CanUseComponent<...>` is not satisfied"
- ✓ "missing field `height` required by CGP component"

**2. CGP-Aware Terminology**

Use terms that match user mental models:
- **Components**: `AreaCalculatorComponent` 
- **Providers**: `RectangleArea` implements `AreaCalculator`
- **Check traits**: `CanUseRectangle` verifies component availability
- **Fields**: Required by components via HasField

Avoid compiler internals:
- ✗ "Symbol<6, Chars<'h', Chars<'e', ...>>>"
- ✓ "field `height`"

**3. Actionable Guidance**

Tell users exactly what to fix:
- "Add `pub height: f64` to the `Rectangle` struct"
- "Add `#[derive(HasField)]` to `Rectangle` if missing"
- "Configure component `X` in delegate_components! macro"

**4. Progressive Disclosure**

Structure information from most to least important:
1. **What:** Missing field
2. **Why:** Required by component
3. **How:** Delegation chain showing requirements
4. **Fix:** Specific actions to resolve

**5. Conciseness**

Show relevant delegation chain, not every trait resolution step:
- ✓ 3-4 key steps from user code to root cause
- ✗ 10+ intermediate trait bounds

### 5.2 Message Structure

#### Template for Missing Field Errors

```
error[E0277]: missing field `<FIELD>` required by CGP component

  <SOURCE_CONTEXT>

  help: <FIELD_WARNING>
        The struct `<TYPE>` is missing the required field `<FIELD>`.
        This field is required by the component `<COMPONENT>`.
        The check trait `<CHECK_TRAIT>` verifies that all required components are available.
        
        Dependency chain:
          → <CHAIN_ENTRY_1>
          → <CHAIN_ENTRY_2>
          → ...
        
        To fix this error:
          - <FIX_SUGGESTION_1>
          - <FIX_SUGGESTION_2>
```

#### Components Explained

**SOURCE_CONTEXT:**
Source code snippet with span highlighting:
```
  ,-[src/base_area.rs:41:9]
40|     CanUseRectangle for Rectangle {
41|         AreaCalculatorComponent,
   :         ^^^^^^^^^^^|^^^^^^^^^^^
   :                    `-- unsatisfied trait bound
42|     }
  `----
```

**FIELD_WARNING:**
If field name contains `�` (unknown character):
```
note: some characters in the field name are hidden by the compiler and shown as '�'
```

**FIX_SUGGESTION:**
- If `has_other_hasfield_impls == true`:
  - "Add the field `<FIELD>` to the `<TYPE>` struct"
  
- If `has_other_hasfield_impls == false`:
  - "Add `#[derive(HasField)]` to the `<TYPE>` struct, if missing"
  - "Ensure the field `<FIELD>` exists in the struct"

#### Delegation Chain Formatting

Transform compiler notes into concise chain:

**Input (from compiler):**
```
required for `Rectangle` to implement `HasRectangleFields`
required for `RectangleArea` to implement `IsProviderFor<AreaCalculatorComponent, Rectangle>`
required for `Rectangle` to implement `cgp::prelude::CanUseComponent<AreaCalculatorComponent>`
```

**Output (formatted):**
```
Dependency chain:
  → required for `Rectangle` to implement `HasRectangleFields`
  → required for `RectangleArea` to implement the provider trait `AreaCalculator`
  → required for `Rectangle` to implement `use component `AreaCalculatorComponent>`
```

**Transformations applied:**
1. Add `→` prefix for visual hierarchy
2. Replace `IsProviderFor<Component, Context>` with "the provider trait `DerivedName`"
3. Replace `CanUseComponent<Component>` with "use component `Component`"
4. Strip module prefixes (`cgp::prelude::` → ``)

**For higher-order providers:**
Add explanatory note before chain:
```
Dependency chain:
  → The error in `ScaledArea<RectangleArea>` is caused by the inner provider `RectangleArea`
  → required for `ScaledArea<RectangleArea>` to implement the provider trait `AreaCalculator`
  → ...
```

#### Example: Complete Output

```
E0277

  × missing field `height` required by CGP component
    ╭─[examples/src/scaled_area.rs:58:9]
 57 │     CanUseRectangle for Rectangle {
 58 │         AreaCalculatorComponent,
    ·         ───────────┬───────────
    ·                    ╰── unsatisfied trait bound
 59 │     }
    ╰────
  help: The struct `Rectangle` is missing the required field `height`.
        This field is required by the component `AreaCalculatorComponent`.
        The check trait `CanUseRectangle` verifies that all required components are available.
        
        Dependency chain:
          → The error in `ScaledArea<RectangleArea>` is caused by the inner provider `RectangleArea`
          → required for `ScaledArea<RectangleArea>` to implement the provider trait `AreaCalculator`
          → required for `Rectangle` to implement `use component `AreaCalculatorComponent>`
          → required for `Rectangle` to implement `HasRectangleFields`
        
        To fix this error:
          - Add the field `height` to the `Rectangle` struct
```

### 5.3 Rendering with Miette

**Why miette:**
- Rich terminal output with colors and spans
- Implements `std::error::Error` and custom `Diagnostic` trait
- Better multi-line label handling than ariadne
- Actively maintained

#### CgpDiagnostic Implementation

```rust
use miette::{Diagnostic, LabeledSpan, NamedSource};

#[derive(Debug, Clone)]
pub struct CgpDiagnostic {
    pub message: String,
    pub code: Option<String>,
    pub help: Option<String>,
    pub source_code: Option<NamedSource<String>>,
    pub labels: Vec<LabeledSpan>,
}

impl fmt::Display for CgpDiagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for CgpDiagnostic {}

impl Diagnostic for CgpDiagnostic {
    fn code<'a>(&'a self) -> Option<Box<dyn fmt::Display + 'a>> {
        self.code.as_ref().map(|c| Box::new(c.clone()) as Box<_>)
    }
    
    fn help<'a>(&'a self) -> Option<Box<dyn fmt::Display + 'a>> {
        self.help.as_ref().map(|h| Box::new(h.clone()) as Box<_>)
    }
    
    fn source_code(&self) -> Option<&dyn miette::SourceCode> {
        self.source_code.as_ref().map(|s| s as &dyn miette::SourceCode)
    }
    
    fn labels(&self) -> Option<Box<dyn Iterator<Item = LabeledSpan> + '_>> {
        if self.labels.is_empty() {
            None
        } else {
            Some(Box::new(self.labels.clone().into_iter()))
        }
    }
}
```

#### Building from DiagnosticEntry

```rust
pub fn format_error_message(entry: &DiagnosticEntry) -> Option<CgpDiagnostic> {
    let field_info = entry.field_info.as_ref()?;
    
    // Build main message
    let message = format!(
        "missing field `{}` required by CGP component",
        field_info.field_name
    );
    
    // Build help text with sections
    let mut help_sections = build_help_sections(entry, field_info);
    let help = Some(help_sections.join("\n"));
    
    // Build source code and labels
    let (source_code, labels) = build_source_and_labels(entry);
    
    Some(CgpDiagnostic {
        message,
        code: entry.error_code.clone(),
        help,
        source_code,
        labels,
    })
}
```

#### Source Code Spans

Read actual source file (not just snippet from JSON):

```rust
fn build_source_and_labels(entry: &DiagnosticEntry) 
    -> (Option<NamedSource<String>>, Vec<LabeledSpan>) 
{
    let span = entry.primary_span.as_ref()?;
    
    // Read full source file
    let file_content = std::fs::read_to_string(&span.file_name).ok()?;
    let source_code = NamedSource::new(&span.file_name, file_content.clone());
    
    // Calculate byte offset from line/column
    let byte_offset = calculate_byte_offset(&file_content, span.line_start, span.column_start);
    let span_length = span.column_end - span.column_start;
    
    let labeled_span = LabeledSpan::new_with_span(
        span.label.clone(),
        SourceSpan::new(SourceOffset::from(byte_offset), span_length)
    );
    
    (Some(source_code), vec![labeled_span])
}
```

#### Terminal Output

```rust
pub fn render_diagnostic_plain(diagnostic: &CgpDiagnostic) -> String {
    let handler = GraphicalReportHandler::new_themed(GraphicalTheme::none());
    let mut output = String::new();
    
    match handler.render_report(&mut output, diagnostic) {
        Ok(_) => output,
        Err(_) => format!("error: {}", diagnostic.message),  // Fallback
    }
}
```

For color output, use `GraphicalReportHandler::new()` instead.

---

## Chapter 6: Implementation Notes

### 6.1 Edge Cases and Fallback Strategies

#### Non-CGP Error Passthrough

**Challenge:** Not all errors in a CGP project are CGP-related. Standard Rust errors (syntax, borrow checker, etc.) must pass through unchanged.

**Solution:**
```rust
fn is_cgp_diagnostic(diag: &Diagnostic) -> bool {
    let patterns = ["CanUseComponent", "IsProviderFor", "HasField", 
                    "cgp_component", "delegate_components"];
    patterns.iter().any(|p| diag.message.contains(p) || 
                            diag.children.iter().any(|c| c.message.contains(p)))
}

// In main loop:
if is_cgp_diagnostic(&msg.message) {
    db.add_diagnostic(msg);  // Process as CGP error
} else {
    println!("{}", msg.message.rendered.unwrap_or_default());  // Pass through
}
```

**Risk:** False negatives (missing CGP errors) worse than false positives (treating non-CGP as CGP).

**Mitigation:** Err on side of processing as CGP. If transformation fails, fall back to original format.

#### Incomplete Field Name Information

**Challenge:** Compiler truncates long type names, including Symbol character sequences:
```
HasField<Symbol<6, Chars<'h', Chars<'e', ...>>>>
```

**Solution:**
1. Extract visible characters
2. Compare extracted length vs expected length
3. Mark as incomplete: `field_info.is_complete = false`
4. Use replacement character � for hidden characters
5. Add note in output: "some characters are hidden by the compiler"

**User experience:**
```
missing field `heig�t` (possibly incomplete) required by CGP component
...
note: some characters in the field name are hidden by the compiler and shown as '�'
```

#### Malformed Component Names

**Challenge:** Pattern extraction may match partial constructs:
```
component_info.component_type = "cgp::prelude::IsProviderFor<AreaCalculatorComponent"
```

**Solution:** Validate extracted names before using:
```rust
let clean_component = strip_module_prefixes(&component_info.component_type);

// Don't show malformed extractions
if !clean_component.contains("IsProviderFor<") && 
   !clean_component.contains("CanUseComponent<") {
    help_sections.push(format!("This field is required by the component `{}`.", clean_component));
}
```

#### Missing Source Files

**Challenge:** Diagnostic references file that doesn't exist (possible in workspace scenarios).

**Solution:**
```rust
match std::fs::read_to_string(&span.file_name) {
    Ok(file_content) => {
        // Use actual file content for precise spans
    }
    Err(_) => {
        // Fallback: use text from diagnostic span
        let source_text = span.text.iter()
            .map(|line| line.text.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        
        if source_text.is_empty() {
            return (None, vec![]);  // Can't render without source
        }
        // Use fallback source
    }
}
```

#### Diagnostic Transformation Failure

**Challenge:** Any step in analysis might fail (parsing error, unexpected format, etc.).

**Solution:** Graceful degradation to original error:
```rust
pub fn format_error_message(entry: &DiagnosticEntry) -> Option<CgpDiagnostic> {
    // Attempt transformation...
}

// In render loop:
for entry in db.get_active_entries() {
    match format_error_message(entry) {
        Some(cgp_diag) => println!("{}", render_diagnostic_plain(&cgp_diag)),
        None => println!("{}", entry.original.rendered.unwrap_or_default()),
    }
}
```

**Principle:** Better to show original compiler output than crash or show nothing.

### 6.2 Testing

#### Unit Tests for Pattern Extraction

Test individual pattern matching functions with crafted inputs:

```rust
#[test]
fn test_extract_field_info() {
    let msg = "the trait `HasField<Symbol<6, Chars<'h', Chars<'e', \
               Chars<'i', Chars<'g', Chars<'h', Chars<'t', Nil>>>>>>>>` \
               is not implemented for `Rectangle`";
    
    let info = extract_field_from_hasfield(msg).unwrap();
    assert_eq!(info.field_name, "height");
    assert_eq!(info.target_type, "Rectangle");
    assert!(info.is_complete);
    assert!(!info.has_unknown_chars);
}

#[test]
fn test_extract_provider_relationship() {
    let msg = "required for `RectangleArea` to implement \
               `IsProviderFor<AreaCalculatorComponent, Rectangle>`";
               
    let rel = extract_provider_relationship(msg).unwrap();
    assert_eq!(rel.provider_type, "RectangleArea");
    assert_eq!(rel.component, "AreaCalculatorComponent");
    assert_eq!(rel.context, "Rectangle");
}
```

#### Integration Tests with Captured JSON

Save actual compiler JSON output as test fixtures:

```bash
# Capture error output
cargo check --message-format=json 2>&1 > tests/fixtures/base_area.json
```

Test full pipeline:
```rust
#[test]
fn test_base_area_error() {
    let json = std::fs::read_to_string("tests/fixtures/base_area.json").unwrap();
    let mut db = DiagnosticDatabase::new();
    
    for line in json.lines() {
        if let Ok(message) = serde_json::from_str::<Message>(line) {
            if let Message::CompilerMessage(msg) = message {
                render_message(&msg, &mut db);
            }
        }
    }
    
    let diagnostics = db.render_cgp_diagnostics();
    assert_eq!(diagnostics.len(), 1);  // Should merge duplicates
    
    let rendered = render_diagnostic_plain(&diagnostics[0]);
    assert!(rendered.contains("missing field `height`"));
    assert!(rendered.contains("To fix this error"));
}
```

#### Snapshot Testing

Use `insta` crate for regression testing of output format:

```rust
#[test]
fn test_base_area_snapshot() {
    let outputs = test_cgp_error_from_json("base_area.json");
    assert_eq!(outputs.len(), 1);
    assert_snapshot!(outputs[0]);  // Compares to saved snapshot
}
```

Update snapshots when format changes:
```bash
cargo insta review  # Review and accept changes
```

**Benefits:**
- Catch unintended output regressions
- Visual diff of format changes
- Easy to review what changed

#### Test Coverage

**Must test:**
- Each pattern extraction function (HasField, IsProviderFor, components)
- Deduplication logic (provider nesting, delegation chains)
- Edge cases (incomplete field names, missing files)
- Complete pipeline on real examples

**Test matrix:**
- base_area: Simple missing field
- base_area_2: Missing derive(HasField)
- scaled_area: Higher-order provider with missing field
- scaled_area_2: Higher-order provider with missing field in outer

### 6.3 Future Directions

#### Potential Improvements

**Better root cause location:**
Currently, primary span points to check_components! invocation. Could enhance to:
- Identify struct definition for missing field errors
- Point to delegate_components! for misconfiguration
- Requires more sophisticated span analysis

**Multi-error summary:**
When multiple independent errors exist:
```
Found 3 errors in CGP configuration:
  1. Missing field `height` in Rectangle (src/shapes.rs:25)
  2. Missing field `radius` in Circle (src/shapes.rs:40)
  3. Misconfigured provider for AuthComponent (src/app.rs:100)
```

**Interactive mode:**
```bash
cargo cgp check --interactive
# Shows errors one at a time with options:
# [f]ix automatically  [s]kip  [q]uit
```

**IDE integration:**
Generate Language Server Protocol (LSP) diagnostics for real-time feedback in editors.

#### Deployment

**Distribution:**
Publish to crates.io as cargo-cgp crate.

**Installation:**
```bash
cargo install cargo-cgp
```

**Usage:**
```bash
cargo cgp check          # Like cargo check
cargo cgp build          # Like cargo build
cargo cgp test           # Like cargo test
```

**Documentation:**
- README with examples
- docs.rs API documentation
- Tutorial showing before/after error messages

**Maintenance:**
- Track Rust compiler versions for format compatibility
- Collect user feedback via GitHub issues
- Regular updates as CGP patterns evolve
