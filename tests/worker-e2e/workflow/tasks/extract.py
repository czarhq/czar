from czar import task
from pydantic import BaseModel


class Output(BaseModel):
    records: list[str]
    count: int


@task
def main() -> Output:
    records = ["alice", "bob", "carol"]
    return Output(records=records, count=len(records))
