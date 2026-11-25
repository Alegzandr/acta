# Accepted differences

Examples of the CommonMark 0.31.2 spec (`commonmark <n>`) and the GFM extension examples (`gfm <n>`) whose tree differs from the spec's HTML, each with its reason. Read by `conformance.rs`. The trees compare block kinds, list starts and looseness, task states, code with its language, table alignments and cells, and inline runs with emphasis, strong, strikethrough, code (spaces kept) and links with their titles; an unknown block or inline kind fails the suite.

## CommonMark 0.31.2

- commonmark 5: markdown-rs drops the columns left over from a partial tab before a tab-indented code block inside a container
- commonmark 6: markdown-rs drops the columns left over from a partial tab before a tab-indented code block inside a container
- commonmark 7: markdown-rs drops the columns left over from a partial tab before a tab-indented code block inside a container
- commonmark 21: raw HTML is shown as the text it is, never rendered (components-editor.md 5, HTML)
- commonmark 31: raw HTML is shown as the text it is, never rendered (components-editor.md 5, HTML)
- commonmark 96: front matter is on (components-editor.md 5): a text opening with `---` starts YAML front matter
- commonmark 98: front matter is on (components-editor.md 5): a text opening with `---` starts YAML front matter
- commonmark 138: gpui-base drops the space after a code span that ends in a space (the soft break before `aaa`)
- commonmark 148: raw HTML is shown as the text it is, never rendered (components-editor.md 5, HTML)
- commonmark 149: raw HTML is shown as the text it is, never rendered (components-editor.md 5, HTML)
- commonmark 150: raw HTML is shown as the text it is, never rendered (components-editor.md 5, HTML)
- commonmark 151: raw HTML is shown as the text it is, never rendered (components-editor.md 5, HTML)
- commonmark 152: raw HTML is shown as the text it is, never rendered (components-editor.md 5, HTML)
- commonmark 153: raw HTML is shown as the text it is, never rendered (components-editor.md 5, HTML)
- commonmark 154: raw HTML is shown as the text it is, never rendered (components-editor.md 5, HTML)
- commonmark 155: raw HTML is shown as the text it is, never rendered (components-editor.md 5, HTML)
- commonmark 156: raw HTML is shown as the text it is, never rendered (components-editor.md 5, HTML)
- commonmark 157: raw HTML is shown as the text it is, never rendered (components-editor.md 5, HTML)
- commonmark 158: raw HTML is shown as the text it is, never rendered (components-editor.md 5, HTML)
- commonmark 159: raw HTML is shown as the text it is, never rendered (components-editor.md 5, HTML)
- commonmark 160: raw HTML is shown as the text it is, never rendered (components-editor.md 5, HTML)
- commonmark 161: raw HTML is shown as the text it is, never rendered (components-editor.md 5, HTML)
- commonmark 162: raw HTML is shown as the text it is, never rendered (components-editor.md 5, HTML)
- commonmark 163: raw HTML is shown as the text it is, never rendered (components-editor.md 5, HTML)
- commonmark 164: raw HTML is shown as the text it is, never rendered (components-editor.md 5, HTML)
- commonmark 165: raw HTML is shown as the text it is, never rendered (components-editor.md 5, HTML)
- commonmark 166: raw HTML is shown as the text it is, never rendered (components-editor.md 5, HTML)
- commonmark 167: raw HTML is shown as the text it is, never rendered (components-editor.md 5, HTML)
- commonmark 169: raw HTML is shown as the text it is, never rendered (components-editor.md 5, HTML)
- commonmark 170: raw HTML is shown as the text it is, never rendered (components-editor.md 5, HTML)
- commonmark 171: raw HTML is shown as the text it is, never rendered (components-editor.md 5, HTML)
- commonmark 172: raw HTML is shown as the text it is, never rendered (components-editor.md 5, HTML)
- commonmark 173: raw HTML is shown as the text it is, never rendered (components-editor.md 5, HTML)
- commonmark 174: raw HTML is shown as the text it is, never rendered (components-editor.md 5, HTML)
- commonmark 175: raw HTML is shown as the text it is, never rendered (components-editor.md 5, HTML)
- commonmark 176: raw HTML is shown as the text it is, never rendered (components-editor.md 5, HTML)
- commonmark 177: raw HTML is shown as the text it is, never rendered (components-editor.md 5, HTML)
- commonmark 178: raw HTML is shown as the text it is, never rendered (components-editor.md 5, HTML)
- commonmark 179: raw HTML is shown as the text it is, never rendered (components-editor.md 5, HTML)
- commonmark 180: raw HTML is shown as the text it is, never rendered (components-editor.md 5, HTML)
- commonmark 181: raw HTML is shown as the text it is, never rendered (components-editor.md 5, HTML)
- commonmark 182: raw HTML is shown as the text it is, never rendered (components-editor.md 5, HTML)
- commonmark 183: raw HTML is shown as the text it is, never rendered (components-editor.md 5, HTML)
- commonmark 184: raw HTML is shown as the text it is, never rendered (components-editor.md 5, HTML)
- commonmark 185: raw HTML is shown as the text it is, never rendered (components-editor.md 5, HTML)
- commonmark 186: raw HTML is shown as the text it is, never rendered (components-editor.md 5, HTML)
- commonmark 187: raw HTML is shown as the text it is, never rendered (components-editor.md 5, HTML)
- commonmark 188: raw HTML is shown as the text it is, never rendered (components-editor.md 5, HTML)
- commonmark 189: raw HTML is shown as the text it is, never rendered (components-editor.md 5, HTML)
- commonmark 190: raw HTML is shown as the text it is, never rendered (components-editor.md 5, HTML)
- commonmark 191: raw HTML is shown as the text it is, never rendered (components-editor.md 5, HTML)
- commonmark 308: raw HTML is shown as the text it is, never rendered (components-editor.md 5, HTML)
- commonmark 309: raw HTML is shown as the text it is, never rendered (components-editor.md 5, HTML)
- commonmark 311: a list loose only by a blank line between items draws tight: gpui-base keeps each item's spread, not the list's (a known gap, components-editor.md 5 says loose lists take the paragraph gap)
- commonmark 313: a list loose only by a blank line between items draws tight: gpui-base keeps each item's spread, not the list's (a known gap, components-editor.md 5 says loose lists take the paragraph gap)
- commonmark 314: a list loose only by a blank line between items draws tight: gpui-base keeps each item's spread, not the list's (a known gap, components-editor.md 5 says loose lists take the paragraph gap)
- commonmark 315: a list loose only by a blank line between items draws tight: gpui-base keeps each item's spread, not the list's (a known gap, components-editor.md 5 says loose lists take the paragraph gap)
- commonmark 318: markdown-rs drops a trailing blank line inside a fenced code block in a list item
- commonmark 326: a list loose only by a blank line between items draws tight: gpui-base keeps each item's spread, not the list's (a known gap, components-editor.md 5 says loose lists take the paragraph gap)
- commonmark 344: raw HTML is shown as the text it is, never rendered (components-editor.md 5, HTML)
- commonmark 475: raw HTML is shown as the text it is, never rendered (components-editor.md 5, HTML)
- commonmark 476: raw HTML is shown as the text it is, never rendered (components-editor.md 5, HTML)
- commonmark 477: raw HTML is shown as the text it is, never rendered (components-editor.md 5, HTML)
- commonmark 574: markdown-rs leaves a nested image's alt text out of the outer image's alt
- commonmark 602: GFM extended autolinks are on (components-editor.md 5); this CommonMark-only example expects the address as plain text
- commonmark 608: GFM extended autolinks are on (components-editor.md 5); this CommonMark-only example expects the address as plain text
- commonmark 611: GFM extended autolinks are on (components-editor.md 5); this CommonMark-only example expects the address as plain text
- commonmark 612: GFM extended autolinks are on (components-editor.md 5); this CommonMark-only example expects the address as plain text
- commonmark 613: raw HTML is shown as the text it is, never rendered (components-editor.md 5, HTML)
- commonmark 614: raw HTML is shown as the text it is, never rendered (components-editor.md 5, HTML)
- commonmark 615: raw HTML is shown as the text it is, never rendered (components-editor.md 5, HTML)
- commonmark 616: raw HTML is shown as the text it is, never rendered (components-editor.md 5, HTML)
- commonmark 623: raw HTML is shown as the text it is, never rendered (components-editor.md 5, HTML)
- commonmark 630: raw HTML is shown as the text it is, never rendered (components-editor.md 5, HTML)
- commonmark 631: raw HTML is shown as the text it is, never rendered (components-editor.md 5, HTML)
- commonmark 642: raw HTML is shown as the text it is, never rendered (components-editor.md 5, HTML)
- commonmark 643: raw HTML is shown as the text it is, never rendered (components-editor.md 5, HTML)

## GFM 0.29 extensions

- gfm 628: markdown-rs follows GitHub today, which no longer links `ftp://` addresses; Acta would not open one anyway
- gfm 652: raw HTML is shown as the text it is, never rendered (components-editor.md 5, HTML)
