# src/extract.py
import requests
from pydantic import BaseModel

class User(BaseModel):
    id: int
    name: str
    email: str

def fetch_users(api_url: str) -> list[User]:
    response = requests.get(f"{api_url}/users")
    return [User(**u) for u in response.json()]
