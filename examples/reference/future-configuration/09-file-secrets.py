import json
import os

@task
def main(input: Input) -> Output:
    with open(os.environ["CZAR_SECRET_SERVICE_ACCOUNT_PATH"]) as f:
        credentials = json.load(f)
    # ...
