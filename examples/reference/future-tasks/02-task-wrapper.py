# tasks/extract.py
from czar import task
from pydantic import BaseModel
from src.extract import fetch_users, User

class Input(BaseModel):
    api_url: str = "https://api.example.com"

class Output(BaseModel):
    users: list[User]

@task
def main(input: Input) -> Output:
    users = fetch_users(input.api_url)
    return Output(users=users)
