import time
import sys

def check_data_quality():
    from kestra import Kestra
    time.sleep(1)
    score = 0.6
    print(f"Data quality score: {score}")
    Kestra.outputs({'quality_score': score})

def process_high_quality():
    time.sleep(1)
    print("Processing high quality data")

def clean_data():
    time.sleep(1)
    print("Cleaning low quality data")
    time.sleep(1)
    print("Processing cleaned low quality data")

def process_partition(partition_id):
    time.sleep(1)
    print(f"Processing {partition_id}")

def combine_results():
    time.sleep(1)
    print("Combined all partition results")

if __name__ == "__main__":
    task = sys.argv[1] if len(sys.argv) > 1 else None
    if task == "check_data_quality":
        check_data_quality()
    elif task == "process_high_quality":
        process_high_quality()
    elif task == "clean_data":
        clean_data()
    elif task and task.startswith("process_partition_"):
        process_partition(task.replace("process_", ""))
    elif task == "combine_results":
        combine_results()
    else:
        print(f"Unknown task: {task}")
        sys.exit(1)
