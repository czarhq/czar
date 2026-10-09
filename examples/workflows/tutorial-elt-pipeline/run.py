from src_pipeline import create_gold_analytics, load_to_bronze, transform_to_silver

DATA_DIR = "data"

for source in ["posts", "comments", "users"]:
    bronze_count = load_to_bronze(source, "2024-01-15", DATA_DIR)
    print(f"Loaded {bronze_count} records to bronze.{source}")

    silver_count = transform_to_silver(source, DATA_DIR)
    print(f"Transformed {silver_count} records to silver.{source}")

# After all sources loaded and transformed
stats = create_gold_analytics(DATA_DIR)
print(f"Analytics: {stats['total_users']} users, {stats['total_posts']} posts, {stats['total_comments']} comments")
