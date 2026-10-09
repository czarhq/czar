@executor
class RetryExecutor:
    def run(self, config_file: str, ctx: Context, upstream: dict = {}) -> TaskOutput:
        for attempt in range(self.max_retries):
            try:
                return self.execute(config_file, ctx, upstream)
            except Exception as e:
                if attempt < self.max_retries - 1:
                    delay = self.initial_delay * (2 ** attempt)
                    time.sleep(delay)
                else:
                    raise
