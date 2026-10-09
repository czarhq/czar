use czar::{task, Context};

#[task]
fn main(input: Input, ctx: Context) -> czar::Result<Output> {
    let rows = process(&input.item)?;

    ctx.log().info(&format!("Processed {} rows", rows));
    ctx.metrics().record("rows_processed", rows as f64, &[]);

    Ok(Output { processed: rows })
}
