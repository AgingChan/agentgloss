# Mermaid rendering suite

## block

```mermaid
block-beta
  columns 3
  A["Spec"] B["Review"] C["Export"]
  A --> B
  B --> C
```

## class

```mermaid
classDiagram
  class Document {
    +source
    +blocks
  }
  class Block {
    +kind
    +source
  }
  class Review {
    +items
  }
  Document --> Block
  Review --> Block
```

## er

```mermaid
erDiagram
  DOCUMENT ||--o{ BLOCK : contains
  REVIEW ||--o{ ITEM : has
  ITEM }o--|| BLOCK : anchors
```

## flowchart_lr_small

```mermaid
flowchart LR
  A[Spec] --> B[Review]
  B --> C[Agent]
```

## flowchart_lr_wide

```mermaid
flowchart LR
  A[Collect requirements] --> B[Draft architecture]
  B --> C[Write implementation plan]
  C --> D[Implement]
  D --> E[Test]
  E --> F[Release]
```

## flowchart_nested

```mermaid
flowchart LR
  subgraph Input
    A[Spec] --> B[Plan]
  end
  subgraph Review
    C[Comment] --> D[Resolve]
  end
  B --> C
  D --> E[Export]
```

## flowchart_td_tall

```mermaid
flowchart TD
  A[Start] --> B[Parse]
  B --> C{Valid?}
  C -->|yes| D[Layout]
  C -->|no| E[Report error]
  D --> F[Render]
  F --> G[Review]
  G --> H{Approved?}
  H -->|yes| I[Export]
  H -->|no| J[Revise]
  J --> B
```

## gantt

```mermaid
gantt
  title Release plan
  dateFormat YYYY-MM-DD
  section Build
  Rename :done, 2026-11-20, 1d
  Release :active, 2026-11-22, 2d
  section Verify
  Linux install :2026-11-22, 1d
  macOS install :2026-11-23, 1d
```

## gitgraph

```mermaid
gitGraph
  commit id: "initial"
  branch dev
  commit id: "table"
  checkout main
  merge dev
  commit id: "release"
```

## mindmap

```mermaid
mindmap
  root((agentgloss))
    Review
      Comments
      Suggestions
    Media
      Images
      Mermaid
```

## pie

```mermaid
pie title Review outcomes
  "Resolved" : 60
  "Open" : 25
  "Stale" : 15
```

## sequence

```mermaid
sequenceDiagram
  participant Human
  participant Agent
  participant Tool
  Human->>Agent: Write spec
  Agent->>Tool: request review
  Tool-->>Human: open TUI
  Human->>Agent: feedback
  Agent->>Tool: resolve items
  Tool-->>Human: export
```

## sequence_tall

```mermaid
sequenceDiagram
  participant A as Author
  participant R as Reviewer
  participant T as Tool
  A->>T: 1. draft
  R->>T: 2. comment
  A->>T: 3. revise
  R->>T: 4. suggest
  A->>T: 5. accept
  R->>T: 6. resolve
  T-->>A: 7. export
  A->>T: 8. apply
  R->>T: 9. verify
  T-->>R: 10. signoff
```

## state

```mermaid
stateDiagram-v2
  [*] --> Draft
  Draft --> Review: submit
  Review --> Changes: feedback
  Changes --> Review: revise
  Review --> Approved: signoff
  Approved --> [*]
```

## timeline

```mermaid
timeline
  title Review rounds
  2026-11-20 : MVP
  2026-11-21 : Table rendering
  2026-11-22 : Rename and release
```

## unicode_long_labels

```mermaid
flowchart LR
  A["Collect requirements from the agent"] --> B["Review with 日本語 and ✅ labels"]
  B --> C["Implement the approved plan"]
```
