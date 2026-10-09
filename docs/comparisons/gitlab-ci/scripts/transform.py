import json
import time

with open("data.json", "r") as f:
    data = json.load(f)

time.sleep(1)
records = data["records"]
transformed = [x * 2 for x in records]
result = {"records": transformed, "source": data["source"], "transformed": True}
print(f"Transformed {len(transformed)} records")

with open("data.json", "w") as f:
    json.dump(result, f)
