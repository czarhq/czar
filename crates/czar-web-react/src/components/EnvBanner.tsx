interface CzarConfig {
  BANNER_TITLE?: string;
  BANNER_COLOR?: string;
}

declare global {
  interface Window {
    __CZAR_CONFIG__?: CzarConfig;
  }
}

export default function EnvBanner() {
  const title = window.__CZAR_CONFIG__?.BANNER_TITLE;
  if (!title) return null;

  const color = window.__CZAR_CONFIG__?.BANNER_COLOR;

  return (
    <div
      className="env-banner"
      style={color ? { backgroundColor: color } : undefined}
    >
      {title}
    </div>
  );
}
