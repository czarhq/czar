import os

@task
def main(input: Input) -> Output:
    log_level = os.getenv("LOG_LEVEL", "info")
    api_url = os.getenv("API_URL")
    # ...
