# Parallel Tasks Example

_Updated at: 2026-05-22_

**What it demonstrates**: Simple parallel execution with dependency synchronization

**Scenario**: Three independent data fetching tasks run in parallel, followed by a combine step that waits for all three to complete.

## Flow

```
fetch_users  ┐
fetch_orders ├─> combine_data (waits for all 3)
fetch_products┘
```

## Tasks

1. **fetch_users** - Simulates fetching user data (1 second)
2. **fetch_orders** - Simulates fetching order data (1 second)
3. **fetch_products** - Simulates fetching product data (1 second)
4. **combine_data** - Combines all fetched data (1 second)

## Key Points

- No data passing between tasks (just dependencies)
- Tests parallel execution capabilities
- Tests dependency synchronization (fan-in pattern)
- Simple to understand and implement

## Expected Behavior

- Tasks 1-3 should run in parallel
- Task 4 should only start after all three complete
- Total runtime should be ~2 seconds (not 4)
