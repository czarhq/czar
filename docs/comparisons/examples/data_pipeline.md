# Data Pipeline Example

_Updated at: 2026-05-22_

**What it demonstrates**: Data passing between tasks in a simple ETL pipeline

**Scenario**: Extract data from a source, transform it, then load it to a destination.

## Flow

```
extract → transform → load
```

## Tasks

1. **extract** - Extracts raw data
   - Returns: `{"records": [1, 2, 3, 4, 5], "source": "api"}`

2. **transform** - Transforms the extracted data
   - Receives: output from extract
   - Doubles each record value
   - Returns: `{"records": [2, 4, 6, 8, 10], "source": "api", "transformed": True}`

3. **load** - Loads the transformed data
   - Receives: output from transform
   - Prints final data and record count

## Key Points

- Tests data passing between tasks
- Shows how each framework handles serialization
- Demonstrates typed data flow
- Simple linear pipeline (no branching)

## Expected Behavior

- Tasks run sequentially
- Data flows from extract → transform → load
- Final output shows doubled values: [2, 4, 6, 8, 10]
