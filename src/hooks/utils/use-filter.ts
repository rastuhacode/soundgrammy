import { useMemo } from 'react'

interface UseFilterOptions extends Intl.CollatorOptions {
  locale?: string
}

export function useFilter(options: UseFilterOptions = { sensitivity: 'base' }) {
  const collator = useMemo(() => {
    // Falls back to browser default locale if none provided
    return new Intl.Collator(options.locale, options)
  }, [options])

  return useMemo(() => {
    // Intl.Collator natively performs equality checking via compare() === 0
    const match = (string: string, substring: string) => {
      if (!substring) return true

      // For substring matching (contains), we can loop through the string length
      const stringLength = string.length
      const subLength = substring.length

      for (let i = 0; i <= stringLength - subLength; i++) {
        const slice = string.slice(i, i + subLength)
        if (collator.compare(slice, substring) === 0) {
          return true
        }
      }
      return false
    }

    return {
      contains: match,
      startsWith: (string: string, substring: string) =>
        collator.compare(string.slice(0, substring.length), substring) === 0,
      endsWith: (string: string, substring: string) =>
        collator.compare(string.slice(-substring.length), substring) === 0,
    }
  }, [collator])
}
