import json
import sys
import time

time.sleep(1)
data = json.loads(sys.argv[1])
print(f"Loaded {len(data['records'])} records from {data['source']}")
print(f"Final data: {data['records']}")
