select
  ordered_at as order_day,
  round(sum(amount), 2) as total_revenue,
  count(*) as order_count
from {{ ref('stg_orders') }}
group by 1
