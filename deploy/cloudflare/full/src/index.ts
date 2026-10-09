import { env } from "cloudflare:workers";
import { Container, getContainer } from "@cloudflare/containers";

export class CzarFullApp extends Container {
  defaultPort = 8080;
  sleepAfter = "15m";
  envVars = {
    CZAR_DB_BACKEND: env.CZAR_DB_BACKEND ?? "d1",
    CZAR_BOOTSTRAP_WORKFLOW_DEFINITIONS:
      env.CZAR_BOOTSTRAP_WORKFLOW_DEFINITIONS ?? "1",
    CZAR_D1_ACCOUNT_ID: env.CZAR_D1_ACCOUNT_ID,
    CZAR_D1_DATABASE_ID: env.CZAR_D1_DATABASE_ID,
    CZAR_D1_API_TOKEN: env.CZAR_D1_API_TOKEN,
    ...(env.CZAR_D1_BASE_URL ? { CZAR_D1_BASE_URL: env.CZAR_D1_BASE_URL } : {}),
    CZAR_WORKFLOW_DEFINITIONS_DIR:
      env.CZAR_WORKFLOW_DEFINITIONS_DIR ?? "/app/cloudflare-workflows",
    RUST_LOG: env.CZAR_FULL_RUST_LOG ?? "warn",
  };
}

type Env = {
  ASSETS: Fetcher;
  BASIC_AUTH_PASS?: string;
  BASIC_AUTH_USER?: string;
  CZAR_DB_BACKEND?: string;
  CZAR_D1_ACCOUNT_ID?: string;
  CZAR_D1_API_TOKEN?: string;
  CZAR_D1_BASE_URL?: string;
  CZAR_D1_DATABASE_ID?: string;
  CZAR_BOOTSTRAP_WORKFLOW_DEFINITIONS?: string;
  CZAR_FULL_RUST_LOG?: string;
  CZAR_WORKFLOW_DEFINITIONS_DIR?: string;
  CZAR_FULL_APP: DurableObjectNamespace<CzarFullApp>;
};

const AUTH_REALM = "Czar Full";

function unauthorizedResponse() {
  return new Response("Authentication required", {
    status: 401,
    headers: {
      "WWW-Authenticate": `Basic realm="${AUTH_REALM}"`,
      "Cache-Control": "no-store",
    },
  });
}

function unauthorizedConfigResponse() {
  return new Response("Cloudflare auth secrets are not configured", {
    status: 503,
    headers: {
      "Cache-Control": "no-store",
    },
  });
}

function backendConfigResponse() {
  return new Response("Cloudflare D1 runtime configuration is not complete", {
    status: 503,
    headers: {
      "Cache-Control": "no-store",
    },
  });
}

function isAuthorized(request: Request, env: Env) {
  if (!env.BASIC_AUTH_USER || !env.BASIC_AUTH_PASS) {
    return null;
  }

  const authHeader = request.headers.get("authorization");
  if (!authHeader?.startsWith("Basic ")) {
    return false;
  }

  const encoded = authHeader.slice("Basic ".length);
  let decoded: string;
  try {
    decoded = atob(encoded);
  } catch {
    return false;
  }

  const separator = decoded.indexOf(":");
  if (separator === -1) {
    return false;
  }

  const username = decoded.slice(0, separator);
  const password = decoded.slice(separator + 1);
  return username === env.BASIC_AUTH_USER && password === env.BASIC_AUTH_PASS;
}

function shouldProxyToContainer(pathname: string) {
  return pathname === "/ws" || pathname === "/api" || pathname.startsWith("/api/");
}

function hasD1RuntimeConfig(env: Env) {
  return (
    (env.CZAR_DB_BACKEND ?? "d1") === "d1" &&
    Boolean(env.CZAR_D1_ACCOUNT_ID) &&
    Boolean(env.CZAR_D1_DATABASE_ID) &&
    Boolean(env.CZAR_D1_API_TOKEN)
  );
}

function withRestrictedHeaders(response: Response) {
  if (response.status === 101) {
    return response;
  }

  const headers = new Headers(response.headers);
  headers.set("Cache-Control", "private, no-store");
  const vary = headers.get("Vary");
  headers.set("Vary", vary ? `${vary}, Authorization` : "Authorization");
  return new Response(response.body, {
    headers,
    status: response.status,
    statusText: response.statusText,
  });
}

export default {
  async fetch(request: Request, env: Env): Promise<Response> {
    const authResult = isAuthorized(request, env);
    if (authResult === null) {
      return unauthorizedConfigResponse();
    }
    if (!authResult) {
      return unauthorizedResponse();
    }
    if (!hasD1RuntimeConfig(env)) {
      return backendConfigResponse();
    }

    const url = new URL(request.url);
    if (shouldProxyToContainer(url.pathname)) {
      const container = getContainer(env.CZAR_FULL_APP, "main");
      return withRestrictedHeaders(await container.fetch(request));
    }

    return withRestrictedHeaders(await env.ASSETS.fetch(request));
  },
};
