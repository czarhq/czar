# src_pipeline.py
import json
import tempfile
from pathlib import Path

import duckdb
import requests


def load_to_bronze(source: str, date: str, data_dir: str) -> int:
    """Load raw JSONPlaceholder data into the source's bronze table."""
    records = _fetch_records(source)

    Path(data_dir).mkdir(parents=True, exist_ok=True)
    con = duckdb.connect(_source_db(data_dir, source))
    _prepare_database(con)

    with tempfile.NamedTemporaryFile(mode="w", suffix=".json", delete=False) as handle:
        json.dump(records, handle)
        temp_path = handle.name

    try:
        con.execute(
            f"""
            CREATE OR REPLACE TABLE bronze.{source} AS
            SELECT
                *,
                TIMESTAMP '{date} 00:00:00' AS loaded_at,
                DATE '{date}' AS batch_date
            FROM read_json_auto(?)
            """,
            [temp_path],
        )
        count = con.execute(f"SELECT COUNT(*) FROM bronze.{source}").fetchone()[0]
    finally:
        con.close()
        Path(temp_path).unlink(missing_ok=True)

    return count


def transform_to_silver(source: str, data_dir: str) -> int:
    """Normalize the source's bronze table into a cleaned silver table."""
    con = duckdb.connect(_source_db(data_dir, source))
    _prepare_database(con)

    if source == "posts":
        con.execute(
            """
            CREATE OR REPLACE TABLE silver.posts AS
            SELECT
                CAST(id AS INTEGER) AS post_id,
                CAST(userId AS INTEGER) AS user_id,
                TRIM(title) AS title,
                TRIM(body) AS body,
                loaded_at,
                batch_date
            FROM bronze.posts
            WHERE id IS NOT NULL
            """
        )
    elif source == "comments":
        con.execute(
            """
            CREATE OR REPLACE TABLE silver.comments AS
            SELECT
                CAST(id AS INTEGER) AS comment_id,
                CAST(postId AS INTEGER) AS post_id,
                TRIM(name) AS name,
                LOWER(TRIM(email)) AS email,
                TRIM(body) AS body,
                loaded_at,
                batch_date
            FROM bronze.comments
            WHERE id IS NOT NULL
            """
        )
    elif source == "users":
        con.execute(
            """
            CREATE OR REPLACE TABLE silver.users AS
            SELECT
                CAST(id AS INTEGER) AS user_id,
                TRIM(name) AS name,
                TRIM(username) AS username,
                LOWER(TRIM(email)) AS email,
                loaded_at,
                batch_date
            FROM bronze.users
            WHERE id IS NOT NULL
            """
        )
    else:
        raise ValueError(f"Unsupported source '{source}'")

    result = con.execute(f"SELECT COUNT(*) FROM silver.{source}").fetchone()[0]
    con.close()
    return result


def create_gold_analytics(data_dir: str) -> dict:
    """Join the sources' silver tables into gold tables; return summary metrics."""
    con = duckdb.connect(str(Path(data_dir) / "warehouse.duckdb"))
    _prepare_database(con)
    for source in ["users", "posts", "comments"]:
        con.execute(f"ATTACH '{_source_db(data_dir, source)}' AS {source}_db (READ_ONLY)")

    con.execute(
        """
        CREATE OR REPLACE TABLE gold.user_engagement AS
        SELECT
            u.user_id,
            u.name,
            u.username,
            COUNT(DISTINCT p.post_id) AS post_count,
            COUNT(DISTINCT c.comment_id) AS comment_count,
            COUNT(DISTINCT c.comment_id) * 1.0 / NULLIF(COUNT(DISTINCT p.post_id), 0) AS avg_comments_per_post
        FROM users_db.silver.users u
        LEFT JOIN posts_db.silver.posts p ON u.user_id = p.user_id
        LEFT JOIN comments_db.silver.comments c ON p.post_id = c.post_id
        GROUP BY u.user_id, u.name, u.username
        """
    )

    con.execute(
        """
        CREATE OR REPLACE TABLE gold.top_posts AS
        SELECT
            p.post_id,
            p.title,
            u.name AS author,
            COUNT(c.comment_id) AS comment_count
        FROM posts_db.silver.posts p
        JOIN users_db.silver.users u ON p.user_id = u.user_id
        LEFT JOIN comments_db.silver.comments c ON p.post_id = c.post_id
        GROUP BY p.post_id, p.title, u.name
        ORDER BY comment_count DESC
        LIMIT 10
        """
    )

    stats = con.execute(
        """
        SELECT
            (SELECT COUNT(*) FROM users_db.silver.users) AS total_users,
            (SELECT COUNT(*) FROM posts_db.silver.posts) AS total_posts,
            (SELECT COUNT(*) FROM comments_db.silver.comments) AS total_comments,
            (SELECT AVG(comment_count) FROM gold.user_engagement) AS avg_comments_per_user
        """
    ).fetchone()
    con.close()

    return {
        "total_users": stats[0],
        "total_posts": stats[1],
        "total_comments": stats[2],
        "avg_comments_per_user": round(stats[3] or 0.0, 2),
    }


def _fetch_records(source: str) -> list[dict]:
    response = requests.get(
        f"https://jsonplaceholder.typicode.com/{source}",
        timeout=30,
    )
    response.raise_for_status()
    records = response.json()
    if not isinstance(records, list):
        raise ValueError(f"Expected a list of records for source '{source}'")
    return records


def _source_db(data_dir: str, source: str) -> str:
    # One DuckDB file per source: DuckDB lets one process write a file at a
    # time, so the sources can load in parallel only into separate files.
    return str(Path(data_dir) / f"{source}.duckdb")


def _prepare_database(con: duckdb.DuckDBPyConnection) -> None:
    con.execute("CREATE SCHEMA IF NOT EXISTS bronze")
    con.execute("CREATE SCHEMA IF NOT EXISTS silver")
    con.execute("CREATE SCHEMA IF NOT EXISTS gold")
