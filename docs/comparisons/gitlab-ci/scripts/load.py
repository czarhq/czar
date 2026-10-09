import json
import time

with open("data.json", "r") as f:
    data = json.load(f)

time.sleep(1)
print(f"Loaded {len(data['records'])} records from {data['source']}")
print(f"Final data: {data['records']}")
