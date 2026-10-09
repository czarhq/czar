-- sql/staging/users.sql
CREATE OR REPLACE TABLE {{ target_table }} AS
SELECT *
FROM source.users
WHERE updated_at > DATE_SUB(CURRENT_DATE(), INTERVAL {{ days_back }} DAY)
  AND status = '{{ status }}'
