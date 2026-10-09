import json
import time

time.sleep(1)
data = {"records": [1, 2, 3, 4, 5], "source": "api"}
print(f"Extracted {len(data['records'])} records")

with open("data.json", "w") as f:
    json.dump(data, f)
