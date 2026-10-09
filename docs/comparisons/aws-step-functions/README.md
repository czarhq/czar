# AWS Step Functions Examples

_Updated at: 2026-05-22_

AWS serverless workflow orchestration using Amazon States Language (JSON).

## Setup

Requires AWS account and CLI configured:

```bash
aws configure
```

## Deploying

### Create Lambda Functions First

Each state references a Lambda function. Create them:

```bash
# Example: Create a Lambda for fetch_users
zip function.zip index.py  # Your Python code
aws lambda create-function \
  --function-name fetch_users \
  --runtime python3.12 \
  --role arn:aws:iam::ACCOUNT_ID:role/lambda-role \
  --handler index.handler \
  --zip-file fileb://function.zip
```

Repeat for all functions referenced in state machines.

### Deploy State Machine

```bash
# Update ARNs in JSON files with your Lambda ARNs
# Then create state machine
aws stepfunctions create-state-machine \
  --name parallel-tasks-flow \
  --definition file://parallel_tasks.json \
  --role-arn arn:aws:iam::ACCOUNT_ID:role/stepfunctions-role
```

## Running

### Via AWS Console

1. Go to AWS Step Functions console
2. Select your state machine
3. Click "Start execution"
4. View execution flow and logs

### Via AWS CLI

```bash
aws stepfunctions start-execution \
  --state-machine-arn arn:aws:states:REGION:ACCOUNT_ID:stateMachine:parallel-tasks-flow

# Check status
aws stepfunctions describe-execution \
  --execution-arn <execution-arn>
```

## Examples

- `parallel_tasks.json` - Parallel state with branches
- `data_pipeline.json` - Sequential states with data passing
- `conditional_branching.json` - Choice states and Map state for loops

Note: All JSON files contain placeholder ARNs - update with your Lambda function ARNs.

## Documentation

[Step Functions Docs](https://docs.aws.amazon.com/step-functions/)
