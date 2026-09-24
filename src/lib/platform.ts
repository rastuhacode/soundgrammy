export function isAndroid(userAgent: string): boolean {
  return /\bAndroid\b/i.test(userAgent)
}
