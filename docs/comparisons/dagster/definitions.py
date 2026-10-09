from dagster import Definitions

from jobs.conditional_branching import conditional_branching_flow
from jobs.data_pipeline import extract, load, transform
from jobs.parallel_tasks import combine_data, fetch_orders, fetch_products, fetch_users

defs = Definitions(
    assets=[fetch_users, fetch_orders, fetch_products, combine_data, extract, transform, load],
    jobs=[conditional_branching_flow],
)
