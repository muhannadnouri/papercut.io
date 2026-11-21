# PDF Find Unicode-wrap fixture

`find-unicode-wraps.pdf` is an authored, generic two-page PDF, unrelated to private
books. Both pages contain the same ferry sentence, starting after `Prefix 😀 `.
Page 1 splits `Straightfor‐` / `ward` with U+2010; page 2 uses U+2011. A further
line break separates `harbor` / `to`. Helvetica's custom ToUnicode map assigns
bytes 80/81 to those hyphens and byte 82 to the emoji's surrogate pair.

`src/viewers/pdfFind.integration.test.ts` parses the actual bytes with PDF.js and
checks Papercut's Find integration against the untouched text-item offsets. Keep
this fixture local/offline and stable. Its text streams and ToUnicode map are
uncompressed ASCII; Git treats this fixture as binary to preserve bytes on CRLF
checkouts. Changing the streams requires recalculating stream lengths and xref
byte offsets. No copyrighted sample files or external downloads are required.
