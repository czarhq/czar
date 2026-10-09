import time

time.sleep(1)
score = 0.6
print(f"Data quality score: {score}")
is_high = "true" if score > 0.8 else "false"
print(f"score={score}")
print(f"is_high_quality={is_high}")
