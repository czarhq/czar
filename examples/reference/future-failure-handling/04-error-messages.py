@task
def main(input: Input) -> Output:
    try:
        result = risky_operation()
        return Output(result=result)
    except ValueError as e:
        raise TaskError(
            message="Invalid input data",
            details={"error": str(e), "input": input.dict()}
        )
