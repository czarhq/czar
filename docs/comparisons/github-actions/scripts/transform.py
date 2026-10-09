import json
import sys
import time

time.sleep(1)
input_data = json.loads(sys.argv[1])
records = input_data["records"]
transformed = [x * 2 for x in records]
result = {"records": transformed, "source": input_data["source"], "transformed": True}
print(f"Transformed {len(transformed)} records")
print(json.dumps(result))
