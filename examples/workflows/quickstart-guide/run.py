from src.data_ops import fetch_data, summarize_data

URL = "https://jsonplaceholder.typicode.com/posts/1"

# Fetch
data = fetch_data(URL)
print(f"Downloaded {len(data)} bytes")

# Process
stats = summarize_data(data)
print(f"Stats: {stats}")
