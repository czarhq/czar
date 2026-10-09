# Complete Survey of Workflow Orchestration Frameworks (2026)

_Updated at: 2026-05-22_

Comprehensive list of data engineering, MLOps, and workflow orchestration frameworks with GitHub stars and categories. Star counts are a point-in-time snapshot and drift over time.

## Summary Statistics

- **Total frameworks surveyed:** 24
- **Categories:** General Orchestration (8), Data Engineering (6), MLOps-Specific (4), CI/CD (3), Low-Code/Visual (3)
- **Star range:** 4.1k - 168k stars
- **Most popular by stars:** n8n (168k), Airflow (43.8k), Kestra (26.2k)
- **Most popular by actual usage:** Airflow (320M downloads, 90% recommend), MLflow (most widely adopted MLOps)

## Complete Framework List

### General Workflow Orchestration

| Framework | GitHub | Stars | Language | Year | Description |
|-----------|--------|-------|----------|------|-------------|
| **n8n** | [n8n-io/n8n](https://github.com/n8n-io/n8n) | ⭐ 168k | TypeScript | 2019 | Fair-code workflow automation with visual editor and 400+ integrations |
| **Airflow** | [apache/airflow](https://github.com/apache/airflow) | ⭐ 43.8k | Python | 2015 | Apache Foundation, most mature, Python-based DAG orchestration |
| **Kestra** | [kestra-io/kestra](https://github.com/kestra-io/kestra) | ⭐ 26.2k | Java | 2020 | Event-driven YAML-first orchestration with modern UI |
| **Prefect** | [PrefectHQ/prefect](https://github.com/PrefectHQ/prefect) | ⭐ 21.3k | Python | 2018 | Python-first, modern Airflow alternative with optional server |
| **Luigi** | [spotify/luigi](https://github.com/spotify/luigi) | ⭐ 18.4k | Python | 2012 | Spotify's batch job framework, handles dependencies and visualization |
| **Temporal** | [temporalio/temporal](https://github.com/temporalio/temporal) | ⭐ 17.5k | Go | 2019 | Durable execution engine for saga patterns and long-running workflows |
| **Argo Workflows** | [argoproj/argo-workflows](https://github.com/argoproj/argo-workflows) | ⭐ 16.3k | Go | 2017 | Kubernetes-native workflow engine, CNCF graduated project |
| **Dagster** | [dagster-io/dagster](https://github.com/dagster-io/dagster) | ⭐ 14.7k | Python | 2018 | Asset-based orchestration focused on data lineage and observability |

### Data Engineering Focused

| Framework | GitHub | Stars | Language | Year | Description |
|-----------|--------|-------|----------|------|-------------|
| **Kedro** | [kedro-org/kedro](https://github.com/kedro-org/kedro) | ⭐ 10.7k | Python | 2019 | Production-ready data science pipelines, LF AI & Data Foundation |
| **Mage** | [mage-ai/mage-ai](https://github.com/mage-ai/mage-ai) | ⭐ 8.6k | Python | 2021 | Data engineering + MLOps, notebook-style pipelines, modern UI |
| **Apache NiFi** | [apache/nifi](https://github.com/apache/nifi) | ⭐ 5.9k | Java | 2014 | Dataflow automation with visual drag-and-drop interface |
| **Orchest** | [orchest/orchest](https://github.com/orchest/orchest) | ⭐ 4.1k | Python | 2020 | Visual pipeline builder for data science workflows |
| **dbt** | [dbt-labs/dbt-core](https://github.com/dbt-labs/dbt-core) | ⭐ ~35k* | Python | 2016 | SQL transformation framework, not orchestration but often used with orchestrators |
| **SQLMesh** | [TobikoData/sqlmesh](https://github.com/TobikoData/sqlmesh) | ⭐ ~2k* | Python | 2022 | dbt alternative with stronger testing, now owned by Fivetran |

### MLOps-Specific Orchestration

| Framework | GitHub | Stars | Language | Year | Description |
|-----------|--------|-------|----------|------|-------------|
| **Metaflow** | [Netflix/metaflow](https://github.com/Netflix/metaflow) | ⭐ 8.8k | Python | 2019 | Netflix's ML/AI workflow framework, now maintained by Outerbounds |
| **Flyte** | [flyteorg/flyte](https://github.com/flyteorg/flyte) | ⭐ 6.7k | Go | 2020 | Kubernetes-native ML orchestration, built for scalability |
| **ZenML** | [zenml-io/zenml](https://github.com/zenml-io/zenml) | ⭐ ~5k* | Python | 2021 | MLOps framework from pipelines to agents, modular and extensible |
| **Kubeflow Pipelines** | [kubeflow/pipelines](https://github.com/kubeflow/pipelines) | ⭐ 4.0k | Python | 2018 | Google-originated ML pipelines for Kubernetes |

### CI/CD and Code-First Orchestration

| Framework | GitHub | Stars | Language | Year | Description |
|-----------|--------|-------|----------|------|-------------|
| **GitHub Actions** | N/A (proprietary) | N/A | YAML | 2019 | GitHub's integrated CI/CD, 71M jobs/day, adding parallel steps in 2026 |
| **GitLab CI** | [gitlabhq/gitlabhq](https://github.com/gitlabhq/gitlabhq) | ⭐ 24.2k | Ruby | 2011 | GitLab's integrated CI/CD with advanced DAG pipelines |
| **Jenkins** | [jenkinsci/jenkins](https://github.com/jenkinsci/jenkins) | ⭐ ~23k* | Java | 2011 | Classic CI/CD with massive plugin ecosystem, still widely used |

### Cloud-Managed Services

| Framework | Provider | Cost Model | Year | Description |
|-----------|----------|------------|------|-------------|
| **GCP Workflows** | Google Cloud | Pay-per-execution | 2020 | Serverless YAML-based orchestration for GCP services |
| **AWS Step Functions** | AWS | Pay-per-state-transition | 2016 | Serverless workflow coordination for AWS services |
| **Azure Data Factory** | Microsoft Azure | Pay-per-activity | 2015 | Cloud ETL and data integration service |
| **Databricks Workflows** | Databricks | Included in platform | 2021 | Native orchestration for Databricks jobs and notebooks |

### Low-Code/Visual Platforms

| Framework | GitHub | Stars | Type | Description |
|-----------|--------|-------|------|-------------|
| **n8n** | [n8n-io/n8n](https://github.com/n8n-io/n8n) | ⭐ 168k | Open-source | Fair-code automation with visual workflow builder |
| **Zapier** | N/A (proprietary) | N/A | SaaS | Most popular no-code automation, 6000+ integrations |
| **Make (Integromat)** | N/A (proprietary) | N/A | SaaS | Visual automation platform, strong in Europe |

*Note: Approximate stars, exact count not available in search results

## Framework Categories by Use Case

### Best for Data Engineering Pipelines
1. **Airflow** - Industry standard, massive ecosystem
2. **Prefect** - Modern alternative, easier to use
3. **Dagster** - Best for data lineage and observability
4. **Kestra** - YAML-first, event-driven
5. **dbt** - SQL transformations (needs orchestrator)

### Best for MLOps
1. **Kubeflow Pipelines** - Kubernetes-native, Google-backed
2. **Metaflow** - Battle-tested at Netflix, simple API
3. **Flyte** - Highly scalable, reproducible
4. **ZenML** - Modular, integrates with many tools
5. **Kedro** - Software engineering best practices

### Best for Kubernetes
1. **Argo Workflows** - CNCF graduated, Kubernetes-native
2. **Kubeflow Pipelines** - ML on Kubernetes
3. **Flyte** - Kubernetes-native with strong typing
4. **Airflow** - Can run on Kubernetes (KubernetesExecutor)

### Best for Serverless/Low Infrastructure
1. **GCP Workflows** - Fully managed, pay-per-execution
2. **AWS Step Functions** - AWS-native serverless
3. **Prefect Cloud** - Managed service with hybrid execution
4. **Databricks Workflows** - Integrated with Databricks platform

### Best for Visual/Low-Code
1. **n8n** - 168k stars, self-hostable, 400+ integrations
2. **Apache NiFi** - Enterprise-grade visual dataflow
3. **Kestra** - Modern UI with YAML + visual editor
4. **Zapier/Make** - SaaS, easiest for non-technical users

### Best for CI/CD
1. **GitHub Actions** - 71M jobs/day, native GitHub integration
2. **GitLab CI** - Advanced DAGs, on-prem support
3. **Jenkins** - Maximum flexibility, huge plugin ecosystem
4. **Argo Workflows** - GitOps-friendly, Kubernetes-native

## Key Trends (2026)

### 1. Convergence of Data and ML Orchestration
- MLOps tools adding general orchestration (ZenML, Flyte)
- Data tools adding ML features (Dagster, Prefect)
- Boundaries blurring between categories

### 2. Kubernetes-Native Growth
- Argo Workflows: 16.3k stars (CNCF graduated)
- Flyte, Kubeflow gaining traction
- Airflow adding better K8s support

### 3. Python-First vs YAML-First Divide
**Python-First:** Prefect (21.3k), Dagster (14.7k), Metaflow (8.8k) **YAML-First:** Kestra (26.2k), GCP Workflows, Argo (16.3k)

Both approaches viable, depends on team preference.

### 4. Serverless and Managed Services
- Cloud providers investing heavily (GCP Workflows, AWS Step Functions, Databricks)
- Reduces operational burden
- Vendor lock-in concerns drive open-source adoption

### 5. Visual/Low-Code Explosion
- n8n: 168k stars (fastest growing)
- Enterprise adoption of visual tools increasing
- Balance between ease-of-use and flexibility

### 6. CI/CD Pricing Pressure
- GitHub Actions introducing self-hosted runner fees (March 2026)
- Driving evaluation of alternatives (GitLab, Argo, etc.)
- Open-source tools gaining traction

## Market Consolidation

### Recent Acquisitions/Mergers
- **Fivetran + dbt Labs** (2025) - Creates dominant ELT + transformation stack
- **Fivetran acquires SQLMesh** (2025) - Brings SQLMesh under same umbrella as dbt
- **Databricks + MosaicML** (2023) - Strengthens ML capabilities

### Impact on Tool Selection
- Vendor lock-in concerns increasing
- Multi-vendor strategies gaining favor
- Open-source tools as hedge against consolidation

## Actual Usage Data - Limited but Available

**Major independent developer surveys don't track orchestration tools:**
- Stack Overflow 2025 (49k devs) - No orchestration category
- JetBrains 2025 (24k devs) - No orchestration category
- Kaggle ML Survey - Limited public data on tools

**Most "surveys" are vendor-funded or lack orchestration data:**
- "State of Airflow" → Astronomer (sells managed Airflow) - obvious bias
- "State of Analytics Engineering" → dbt Labs (sells dbt) - doesn't ask about orchestration
- "State of Enterprise Data and AI" → Atlan - 561 respondents, focuses on AI adoption not tooling
- "O'Reilly DataOps" → Likely vendor-sponsored (Prefect claims)
- MLOps reports → Market research firms selling reports
- Matillion survey → 307 respondents, doesn't specify which orchestration tools

**Independent surveys found:**

### 1. DataTalks.Club Survey (200+ data professionals)
**Sample:** 200+ data professionals involved in data engineering tasks

**Orchestration tool usage:**
- **Airflow: 48.3%** (101 respondents) - clear leader
- **No orchestration: 35.9%** (75 respondents) - significant portion
- **AWS Step Functions: 12%** (25 respondents)
- **Mage: 7.2%** (15 respondents)
- **Prefect: 6.7%** (14 respondents)
- **Dagster: 4.8%** (10 respondents)
- **Others: <2% each** (Beam, GitHub Actions, Jenkins)

**Limitations:** Small sample, methodology not fully disclosed, self-selected audience

**Source:** [How Do Data Professionals Use Data Engineering Tools and Practices?](https://datatalks.club/blog/how-do-data-professionals-use-data-engineering-tools-and-practices.html)

### 2. CNCF/SlashData ML Orchestration Survey (Q3 2025, 300+ developers)
**Sample:** 300+ professional cloud-native developers worldwide

**ML orchestration tool ratings (5-star scale):**
- **Airflow**: "Adopt" tier - highest usefulness ratings, zero 1-2 star ratings
- **Metaflow**: "Adopt" tier - highest maturity (84% gave 4-5 stars)
- **Flyte**: Lower tier - mostly 3-star ratings
- **Seldon Core**: Moderate scores

**Methodology:** Rated on familiarity, usefulness, maturity, recommendation (5-star scale), transformed to net promoter scores

**Source:** [CNCF and SlashData Report on AI Tools in Cloud Native Ecosystems](https://www.cncf.io/announcements/2025/11/11/cncf-and-slashdata-report-finds-leading-ai-tools-gaining-adoption-in-cloud-native-ecosystems/)

### 3. GradientFlow Workflow Orchestration Survey (2022, 581 respondents)
**Sample:** 581 respondents across data scientists/analysts, data/ML engineers, software engineers, DevOps

**Current orchestration tool usage:**
- **Airflow: 36%** overall (46% among data scientists/analysts)
- **Prefect: 14%** overall
- **Other tools:** Not specified with percentages
- **43% use orchestrator** for over half of recurring tasks

**Future interest ("plan to try next"):**
- Among data/ML engineers: Prefect (18%), Dagster (18%), Airflow (15%)
- Among software engineers/DevOps: Prefect (14%)
- **82% of data scientists/analysts** plan to use new orchestration tool next year

**Note:** 2022 data, predates many recent developments.

**Source:** [GradientFlow 2022 Workflow Orchestration Report](https://gradientflow.com/2022-workflow-orchestration-survey/)

**What we can infer from indirect signals:**

### Download/Usage Metrics
- **Airflow:** 320M downloads in 2024 (via multiple sources - Astronomer, PracData)
  - 10x more than nearest competitor
  - 31M downloads in Nov 2024 alone (vs 888k in Nov 2020)
- **MLflow:** "Most widely adopted MLOps" (via market research, no numbers)
- **n8n:** 168k GitHub stars (most starred, but visual automation != data engineering)

### Community Size (GitHub Stars as proxy)
1. n8n (168k) - Visual automation, different audience
2. Airflow (43.8k) - Data/ML engineering
3. Kestra (26.2k) - Growing fast, YAML-first
4. Prefect (21.3k) - Python-first alternative
5. Luigi (18.4k) - Spotify's older framework
6. Temporal (17.5k) - Durable execution niche

### Anecdotal Evidence (Hacker News, Reddit)
- [Ask HN: Simplest data orchestration tool?](https://news.ycombinator.com/item?id=43439939) - Users complain Airflow is "overly complex"
- General consensus: Airflow dominant but complex, Prefect/Dagster gaining for new projects
- r/dataengineering - No recent polls found
- No systematic data available

### Market Research Claims (take with grain of salt)
- Flexera survey: 46% cite integration challenges with orchestration tools
- Gartner: 70% of enterprises plan to invest in AI orchestration (next 2 years)
- MLOps Community: Organizations using right orchestration tools see 37% better project success, 42% faster time-to-value
- Source quality unclear, likely vendor-funded

### Reality Check
Without independent surveys, we only have:
- Biased vendor claims
- GitHub stars (poor proxy for enterprise usage)
- Download counts (inflated by CI/CD, bots)
- Anecdotal community discussions

**Actual enterprise adoption is unknown.**

## Where Czar Fits

- **Stars:** none (not public yet)
- **Category:** general workflow orchestration
- **Profile:** Rust core with Python (and process / container / wasm / library) tasks; YAML workflow definitions; SQLite / Postgres / D1 state backends; `czar serve` runs the scheduler and web UI.
- **Closest comparisons:** Prefect (Python-centric, optional server), Kestra (YAML-first), GCP Workflows (serverless, vendor-locked).
- **No production track record or community ecosystem yet.**

## Sources

**Data Orchestration:**
- [Compare Top 15 Data Orchestration Tools in 2026](https://research.aimultiple.com/data-orchestration-tools/)
- [12 Best Open-Source Data Orchestration Tools in 2026 | Airbyte](https://airbyte.com/top-etl-tools-for-sources/data-orchestration-tools)
- [Top Data Orchestration Tools in 2026](https://www.alation.com/blog/data-orchestration-tools/)

**MLOps:**
- [25 Top MLOps Tools You Need to Know in 2026 | DataCamp](https://www.datacamp.com/blog/top-mlops-tools)
- [27 MLOps Tools for 2026: Key Features & Benefits](https://lakefs.io/blog/mlops-tools/)
- [Best Machine Learning Workflow and Pipeline Orchestration Tools](https://neptune.ai/blog/best-workflow-and-pipeline-orchestration-tools)

**GitHub Repositories:**
- [GitHub - apache/airflow](https://github.com/apache/airflow)
- [GitHub - kestra-io/kestra](https://github.com/kestra-io/kestra)
- [GitHub - PrefectHQ/prefect](https://github.com/PrefectHQ/prefect)
- [GitHub - temporalio/temporal](https://github.com/temporalio/temporal)
- [GitHub - dagster-io/dagster](https://github.com/dagster-io/dagster)
- [GitHub - mage-ai/mage-ai](https://github.com/mage-ai/mage-ai)
- [GitHub - argoproj/argo-workflows](https://github.com/argoproj/argo-workflows)
- [GitHub - flyteorg/flyte](https://github.com/flyteorg/flyte)
- [GitHub - Netflix/metaflow](https://github.com/Netflix/metaflow)
- [GitHub - kedro-org/kedro](https://github.com/kedro-org/kedro)
- [GitHub - spotify/luigi](https://github.com/spotify/luigi)
- [GitHub - kubeflow/pipelines](https://github.com/kubeflow/pipelines)
- [GitHub - apache/nifi](https://github.com/apache/nifi)
- [GitHub - n8n-io/n8n](https://github.com/n8n-io/n8n)
- [GitHub - zenml-io/zenml](https://github.com/zenml-io/zenml)
- [GitHub - orchest/orchest](https://github.com/orchest/orchest)

**CI/CD:**
- [Is Github Actions the ultimate workflow orchestration tool? | Orchestra](https://www.getorchestra.io/blog/is-github-actions-the-ultimate-workflow-orchestration-tool)
- [CI/CD Deep Dive: Why Jenkins, GitLab, and CircleCI Still Rule in 2026](https://dev.to/dataformathub/cicd-deep-dive-why-jenkins-gitlab-and-circleci-still-rule-in-2026-268c)
- [GitHub Actions Pricing: 39% Runner Cuts, New Fees from Jan 2026](https://www.webpronews.com/github-actions-pricing-39-runner-cuts-new-fees-from-jan-2026/)

**dbt Alternatives:**
- [Top 10 dbt Alternatives for Data Transformation in 2026](https://hevodata.com/learn/top-dbt-alternatives-data-transformation/)
- [dbt Alternatives: 10 Platforms Compared | Datacoves](https://datacoves.com/post/dbt-alternatives)
