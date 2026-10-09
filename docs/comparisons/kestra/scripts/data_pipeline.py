import time
import sys

def extract():
    time.sleep(1)
    data = {"records": [1, 2, 3, 4, 5], "source": "api"}
    print(f"Extracted {len(data['records'])} records")

def transform():
    time.sleep(1)
    records = [1, 2, 3, 4, 5]
    transformed = [x * 2 for x in records]
    print(f"Transformed {len(transformed)} records")

def load():
    time.sleep(1)
    records = [2, 4, 6, 8, 10]
    print(f"Loaded {len(records)} records")
    print(f"Final data: {records}")

if __name__ == "__main__":
    task = sys.argv[1] if len(sys.argv) > 1 else None
    if task == "extract":
        extract()
    elif task == "transform":
        transform()
    elif task == "load":
        load()
    else:
        print(f"Unknown task: {task}")
        sys.exit(1)
