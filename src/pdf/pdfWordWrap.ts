// Match the conservative letter-case rule used by PDF search reconstruction.
// Call only at a source line break, never at an ordinary span/word boundary.
export function isPdfWordWrap(before: string, after: string): boolean {
  return (/\p{Ll}[-\u2010\u2011]$/u.test(before) && /^\p{Ll}/u.test(after))
    || (/\p{Lu}[-\u2010\u2011]$/u.test(before) && /^\p{L}/u.test(after))
}
