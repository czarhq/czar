select *
from {{ ref('daily_revenue') }}
where total_revenue <= 0
