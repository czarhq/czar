type Env = {
  DB: D1Database;
  TEST_TOKEN: string;
};

type QueryParam = null | boolean | number | string;

type QueryRequest = {
  sql?: string;
  params?: QueryParam[];
};

function unauthorized() {
  return Response.json(
    {
      success: false,
      errors: [{ message: "unauthorized" }],
      messages: [],
      result: [],
    },
    { status: 401 },
  );
}

function shouldFetchRows(sql: string) {
  const normalized = sql.trimStart().toUpperCase();
  return (
    normalized.startsWith("SELECT") ||
    normalized.startsWith("PRAGMA") ||
    normalized.includes(" RETURNING ")
  );
}

function okEnvelope(result: { success?: boolean; results?: unknown[]; meta?: unknown }) {
  return Response.json({
    success: true,
    errors: [],
    messages: [],
    result: [
      {
        success: result.success ?? true,
        results: result.results ?? [],
        meta:
          result.meta ?? {
            changed_db: false,
            changes: 0,
            duration: 0,
            rows_read: Array.isArray(result.results) ? result.results.length : 0,
            rows_written: 0,
          },
      },
    ],
  });
}

function errorEnvelope(error: unknown) {
  const message = error instanceof Error ? error.message : String(error);
  return Response.json({
    success: false,
    errors: [{ message }],
    messages: [],
    result: [],
  });
}

export default {
  async fetch(request: Request, env: Env): Promise<Response> {
    const url = new URL(request.url);
    if (url.pathname === "/health") {
      return new Response("ok", { status: 200 });
    }

    if (request.headers.get("authorization") !== `Bearer ${env.TEST_TOKEN}`) {
      return unauthorized();
    }

    if (!/^\/accounts\/[^/]+\/d1\/database\/[^/]+\/query$/.test(url.pathname)) {
      return new Response("not found", { status: 404 });
    }

    let payload: QueryRequest;
    try {
      payload = await request.json<QueryRequest>();
    } catch (error) {
      return errorEnvelope(error);
    }

    if (!payload.sql || typeof payload.sql !== "string") {
      return errorEnvelope("missing SQL statement");
    }

    const params = Array.isArray(payload.params) ? payload.params : [];

    try {
      const statement = env.DB.prepare(payload.sql).bind(...params);
      if (shouldFetchRows(payload.sql)) {
        return okEnvelope(await statement.all());
      }
      return okEnvelope(await statement.run());
    } catch (error) {
      return errorEnvelope(error);
    }
  },
};
