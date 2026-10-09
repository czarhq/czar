# src/data_ops.py
import requests

def fetch_data(url: str) -> str:
    """Fetch data from URL"""
    response = requests.get(url, timeout=30)
    response.raise_for_status()
    return response.text

def summarize_data(text: str) -> dict:
    """Compute summary statistics"""
    lines = text.split('\n')

    return {
        "total_chars": len(text),
        "total_lines": len(lines),
        "non_empty_lines": len([l for l in lines if l.strip()])
    }
