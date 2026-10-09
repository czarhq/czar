from czar import task
from pydantic import BaseModel


class Output(BaseModel):
    written: int
    done: bool


@task
def main(uppercased: list[str]) -> Output:
    return Output(written=len(uppercased), done=True)
