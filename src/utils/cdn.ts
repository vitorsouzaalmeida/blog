const CDN_BASE = "https://cdn.vitorsalmeida.com";

export type CdnImageOptions = {
  width?: number;
  quality?: number;
  format?: string;
};

export const cdnImage = (file: string, opts: CdnImageOptions = {}): string => {
  const { width, quality = 50, format = "auto" } = opts;
  const params = [`format=${format}`, `quality=${quality}`];
  if (width) params.push(`width=${width}`);
  return `${CDN_BASE}/cdn-cgi/image/${params.join(",")}/${file}`;
};
