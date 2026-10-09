import time
import sys

def fetch_users():
    time.sleep(1)
    print("Fetched users")

def fetch_orders():
    time.sleep(1)
    print("Fetched orders")

def fetch_products():
    time.sleep(1)
    print("Fetched products")

def combine_data():
    time.sleep(1)
    print("Combined all data")

if __name__ == "__main__":
    task = sys.argv[1] if len(sys.argv) > 1 else None
    if task == "fetch_users":
        fetch_users()
    elif task == "fetch_orders":
        fetch_orders()
    elif task == "fetch_products":
        fetch_products()
    elif task == "combine_data":
        combine_data()
    else:
        print(f"Unknown task: {task}")
        sys.exit(1)
