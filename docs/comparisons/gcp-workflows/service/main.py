import time
from typing import Any

from fastapi import FastAPI

app = FastAPI()


# Parallel Tasks Example
@app.post("/fetch_users")
def fetch_users() -> dict[str, str]:
    time.sleep(1)
    print("Fetched users")
    return {"status": "success", "message": "Fetched users"}


@app.post("/fetch_orders")
def fetch_orders() -> dict[str, str]:
    time.sleep(1)
    print("Fetched orders")
    return {"status": "success", "message": "Fetched orders"}


@app.post("/fetch_products")
def fetch_products() -> dict[str, str]:
    time.sleep(1)
    print("Fetched products")
    return {"status": "success", "message": "Fetched products"}


@app.post("/combine_data")
def combine_data(users: dict[str, Any], orders: dict[str, Any], products: dict[str, Any]) -> dict[str, str]:
    time.sleep(1)
    print("Combined all data")
    return {"status": "success", "message": "Combined all data"}


# Data Pipeline Example
@app.post("/extract")
def extract() -> dict[str, Any]:
    time.sleep(1)
    data = {"records": [1, 2, 3, 4, 5], "source": "api"}
    print(f"Extracted {len(data['records'])} records")
    return data


@app.post("/transform")
def transform(data: dict[str, Any]) -> dict[str, Any]:
    time.sleep(1)
    records = data["records"]
    transformed = [x * 2 for x in records]
    result = {"records": transformed, "source": data["source"], "transformed": True}
    print(f"Transformed {len(transformed)} records")
    return result


@app.post("/load")
def load(data: dict[str, Any]) -> dict[str, str]:
    time.sleep(1)
    print(f"Loaded {len(data['records'])} records from {data['source']}")
    print(f"Final data: {data['records']}")
    return {"status": "success", "message": "Data loaded successfully"}


# Conditional Branching Example
@app.post("/check_data_quality")
def check_data_quality() -> dict[str, float]:
    time.sleep(1)
    score = 0.6
    print(f"Data quality score: {score}")
    return {"score": score}


@app.post("/process_high_quality")
def process_high_quality() -> dict[str, str]:
    time.sleep(1)
    print("Processing high quality data")
    return {"status": "success", "message": "Processed high quality data"}


@app.post("/clean_data")
def clean_data() -> dict[str, str]:
    time.sleep(1)
    print("Cleaning low quality data")
    return {"status": "success", "message": "Data cleaned"}


@app.post("/process_low_quality")
def process_low_quality() -> dict[str, str]:
    time.sleep(1)
    print("Processing cleaned low quality data")
    return {"status": "success", "message": "Processed low quality data"}


@app.post("/process_partition")
def process_partition(partition: str) -> dict[str, str]:
    time.sleep(1)
    print(f"Processing {partition}")
    return {"status": "success", "partition": partition}


@app.post("/combine_results")
def combine_results() -> dict[str, str]:
    time.sleep(1)
    print("Combined all partition results")
    return {"status": "success", "message": "All results combined"}


@app.get("/health")
def health() -> dict[str, str]:
    return {"status": "healthy"}
