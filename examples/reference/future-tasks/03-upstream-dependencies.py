# tasks/transform.py
from czar import task
from pydantic import BaseModel
from src.extract import User
from src.transform import enrich, EnrichedUser

class ExtractOutput(BaseModel):
    users: list[User]

class Input(BaseModel):
    extract: ExtractOutput

class Output(BaseModel):
    users: list[EnrichedUser]

@task
def main(input: Input) -> Output:
    enriched = enrich(input.extract.users)
    return Output(users=enriched)
