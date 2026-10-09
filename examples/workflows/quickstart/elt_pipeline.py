"""ELT Pipeline workflow - TypedDict example"""
import json
import sqlite3
from datetime import date as Date
from pathlib import Path
from typing import Any, TypedDict


DATA_DIR = Path(__file__).resolve().parent / "data"


class BronzeOutput(TypedDict):
    source: str
    count: int


class SilverOutput(TypedDict):
    source: str
    bronze_count: int
    silver_count: int


class GoldOutput(TypedDict):
    total_users: int
    total_posts: int
    total_comments: int
    avg_comments_per_user: float


# Business logic functions
def _load_json(source: str) -> list[dict[str, Any]]:
    path = DATA_DIR / f"{source}.json"
    if not path.exists():
        raise FileNotFoundError(f"missing data file: {path}")
    with path.open("r", encoding="utf-8") as f:
        return json.load(f)


def load_to_bronze(source: str, load_date: Date | str, db_path: str) -> int:
    records = _load_json(source)
    db_path_obj = Path(db_path)
    db_path_obj.parent.mkdir(parents=True, exist_ok=True)

    conn = sqlite3.connect(db_path_obj)
    conn.execute("PRAGMA foreign_keys = ON")

    # Convert date to ISO format string for database storage
    parsed_date = Date.fromisoformat(load_date) if isinstance(load_date, str) else load_date
    date_str = parsed_date.isoformat()

    if source == "posts":
        conn.execute("DROP TABLE IF EXISTS bronze_posts")
        conn.execute(
            """
            CREATE TABLE bronze_posts (
                id INTEGER PRIMARY KEY,
                user_id INTEGER,
                title TEXT,
                body TEXT,
                event_time TEXT,
                loaded_at TEXT
            )
            """
        )
        conn.executemany(
            "INSERT INTO bronze_posts (id, user_id, title, body, event_time, loaded_at) VALUES (?, ?, ?, ?, ?, ?)",
            [
                (
                    int(item["id"]),
                    int(item.get("userId", 0)),
                    item.get("title", "").strip(),
                    item.get("body", "").strip(),
                    item.get("timestamp", ""),
                    date_str,
                )
                for item in records
            ],
        )
    elif source == "comments":
        conn.execute("DROP TABLE IF EXISTS bronze_comments")
        conn.execute(
            """
            CREATE TABLE bronze_comments (
                id INTEGER PRIMARY KEY,
                post_id INTEGER,
                name TEXT,
                email TEXT,
                body TEXT,
                loaded_at TEXT
            )
            """
        )
        conn.executemany(
            "INSERT INTO bronze_comments (id, post_id, name, email, body, loaded_at) VALUES (?, ?, ?, ?, ?, ?)",
            [
                (
                    int(item["id"]),
                    int(item.get("postId", 0)),
                    item.get("name", "").strip(),
                    item.get("email", "").strip(),
                    item.get("body", "").strip(),
                    date_str,
                )
                for item in records
            ],
        )
    elif source == "users":
        conn.execute("DROP TABLE IF EXISTS bronze_users")
        conn.execute(
            """
            CREATE TABLE bronze_users (
                id INTEGER PRIMARY KEY,
                name TEXT,
                username TEXT,
                email TEXT,
                loaded_at TEXT
            )
            """
        )
        conn.executemany(
            "INSERT INTO bronze_users (id, name, username, email, loaded_at) VALUES (?, ?, ?, ?, ?)",
            [
                (
                    int(item["id"]),
                    item.get("name", "").strip(),
                    item.get("username", "").strip(),
                    item.get("email", "").strip(),
                    date_str,
                )
                for item in records
            ],
        )
    else:
        conn.close()
        raise ValueError(f"unsupported source {source}")

    conn.commit()
    conn.close()
    return len(records)


def transform_to_silver(source: str, db_path: str) -> int:
    conn = sqlite3.connect(db_path)
    conn.execute("PRAGMA foreign_keys = ON")

    if source == "posts":
        conn.execute("DROP TABLE IF EXISTS silver_posts")
        conn.execute(
            """
            CREATE TABLE silver_posts AS
            SELECT
                CAST(id AS INTEGER) AS post_id,
                CAST(user_id AS INTEGER) AS user_id,
                TRIM(title) AS title,
                TRIM(body) AS body,
                loaded_at
            FROM bronze_posts
            WHERE id IS NOT NULL
            """
        )
        count = conn.execute("SELECT COUNT(*) FROM silver_posts").fetchone()[0]
    elif source == "comments":
        conn.execute("DROP TABLE IF EXISTS silver_comments")
        conn.execute(
            """
            CREATE TABLE silver_comments AS
            SELECT
                CAST(id AS INTEGER) AS comment_id,
                CAST(post_id AS INTEGER) AS post_id,
                TRIM(name) AS name,
                LOWER(TRIM(email)) AS email,
                TRIM(body) AS body,
                loaded_at
            FROM bronze_comments
            WHERE id IS NOT NULL
            """
        )
        count = conn.execute("SELECT COUNT(*) FROM silver_comments").fetchone()[0]
    elif source == "users":
        conn.execute("DROP TABLE IF EXISTS silver_users")
        conn.execute(
            """
            CREATE TABLE silver_users AS
            SELECT
                CAST(id AS INTEGER) AS user_id,
                TRIM(name) AS name,
                TRIM(username) AS username,
                LOWER(TRIM(email)) AS email,
                loaded_at
            FROM bronze_users
            WHERE id IS NOT NULL
            """
        )
        count = conn.execute("SELECT COUNT(*) FROM silver_users").fetchone()[0]
    else:
        conn.close()
        raise ValueError(f"unsupported source {source}")

    conn.commit()
    conn.close()
    return int(count)


def create_gold_analytics(db_path: str) -> dict[str, float | int]:
    conn = sqlite3.connect(db_path)
    conn.execute("PRAGMA foreign_keys = ON")

    conn.execute("DROP TABLE IF EXISTS gold_user_engagement")
    conn.execute(
        """
        CREATE TABLE gold_user_engagement AS
        SELECT
            u.user_id,
            u.name,
            u.username,
            COUNT(DISTINCT p.post_id) AS post_count,
            COUNT(DISTINCT c.comment_id) AS comment_count,
            CASE
                WHEN COUNT(DISTINCT p.post_id) = 0 THEN 0
                ELSE ROUND(COUNT(DISTINCT c.comment_id) * 1.0 / COUNT(DISTINCT p.post_id), 2)
            END AS avg_comments_per_post
        FROM silver_users u
        LEFT JOIN silver_posts p ON u.user_id = p.user_id
        LEFT JOIN silver_comments c ON p.post_id = c.post_id
        GROUP BY u.user_id, u.name, u.username
        """
    )

    conn.execute("DROP TABLE IF EXISTS gold_top_posts")
    conn.execute(
        """
        CREATE TABLE gold_top_posts AS
        SELECT
            p.post_id,
            p.title,
            u.name AS author,
            COUNT(c.comment_id) AS comment_count
        FROM silver_posts p
        JOIN silver_users u ON p.user_id = u.user_id
        LEFT JOIN silver_comments c ON p.post_id = c.post_id
        GROUP BY p.post_id, p.title, u.name
        ORDER BY comment_count DESC, p.post_id ASC
        LIMIT 10
        """
    )

    stats = conn.execute(
        """
        SELECT
            (SELECT COUNT(*) FROM silver_users) AS total_users,
            (SELECT COUNT(*) FROM silver_posts) AS total_posts,
            (SELECT COUNT(*) FROM silver_comments) AS total_comments,
            (SELECT AVG(comment_count) FROM gold_user_engagement) AS avg_comments_per_user
        """
    ).fetchone()

    conn.commit()
    conn.close()

    return {
        "total_users": int(stats[0] or 0),
        "total_posts": int(stats[1] or 0),
        "total_comments": int(stats[2] or 0),
        "avg_comments_per_user": round(stats[3] or 0.0, 2),
    }


# Task 1: Load Bronze
def bronze_loader(source: str, date: str, db_path: str) -> BronzeOutput:
    """Load data from JSON files into bronze tables"""
    count = load_to_bronze(source, date, db_path)
    return {"source": source, "count": count}


# Task 2: Transform Silver
def silver_transformer(source: str, db_path: str, bronze: BronzeOutput) -> SilverOutput:
    """Transform bronze data into silver tables."""
    bronze_source = bronze.get("source", source)
    bronze_count = bronze.get("count", 0)

    silver_count = transform_to_silver(bronze_source, db_path)
    return {
        "source": bronze_source,
        "bronze_count": bronze_count,
        "silver_count": silver_count,
    }


# Task 3: Gold Analytics
def gold_analytics(
    db_path: str,
    silver_posts: SilverOutput,
    silver_comments: SilverOutput,
    silver_users: SilverOutput,
) -> GoldOutput:
    """Create gold analytics tables and compute summary statistics."""
    _ = (silver_posts, silver_comments, silver_users)
    stats = create_gold_analytics(db_path)
    return stats  # type: ignore
