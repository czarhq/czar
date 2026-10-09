from czar import task
from pydantic import BaseModel


class Output(BaseModel):
    uppercased: list[str]
    count: int


@task
def main(records: list[str]) -> Output:
    uppercased = [r.upper() for r in records]
    return Output(uppercased=uppercased, count=len(uppercased))
