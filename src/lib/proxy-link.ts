/** Build a pasteable `tg://proxy?…` link from raw fields. */
export function buildProxyLink(
  server: string,
  port: number,
  secret: string,
): string {
  const s = server.trim()
  const sec = secret.trim()
  if (!s || !sec || !port) return ''
  return `tg://proxy?server=${s}&port=${port}&secret=${sec}`
}
