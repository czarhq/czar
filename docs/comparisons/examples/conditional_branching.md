# Conditional Branching Example

_Updated at: 2026-05-22_

**What it demonstrates**: Conditional execution, branching logic, and dynamic task generation (loops)

**Scenario**: Check data quality, branch based on quality score, then process data in partitions.

## Flow

```
check_data_quality
    ├─> if quality > 0.8: process_high_quality
    └─> else: clean_data → process_low_quality

Then process partitions ["partition_1", "partition_2", "partition_3"] in parallel

combine_results (waits for all)
```

## Tasks

1. **check_data_quality** - Returns quality score (0.6 for testing)

2. **decide_processing_path** - Branches based on quality score
   - If score > 0.8: go to process_high_quality
   - Else: go to clean_data

3. **process_high_quality** - Direct processing for high quality data

4. **clean_data** - Cleans low quality data

5. **process_low_quality** - Processes cleaned data

6. **get_partitions** - Returns list of partitions to process

7. **process_partition** - Processes a single partition (dynamic, runs 3x in parallel)

8. **combine_results** - Combines all partition results

## Key Points

- Tests conditional execution (if/else branching)
- Tests dynamic task generation (loops/map)
- Shows how frameworks handle complex control flow
- Most challenging example (where ergonomics often break down)

## Expected Behavior

- Quality score is 0.6, so low quality path is taken
- Partition processing runs in parallel (3 instances)
- Final combination waits for all partitions
