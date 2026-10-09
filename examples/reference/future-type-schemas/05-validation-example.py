# tasks/extract.py
class Output(BaseModel):
    users: list[User]

@task
def main() -> Output:
    # BUG: returning wrong type
    return Output(users="invalid")  # Validation fails
