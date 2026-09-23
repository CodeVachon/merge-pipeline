#!/usr/bin/env python3
"""Fail if a built mdBook's own pages link to a page that isn't in the output.

Cheaper than pulling in mdbook-linkcheck (unmaintained, needs a cargo install) or an external
crawler: the whole failure mode this guards against is a broken *internal* link — a renamed page,
a typo'd path, or (as actually happened once while writing this book) a link that survives an
`{{#include}}` from a file outside docs/ and points somewhere the book doesn't have. External
links are not checked here; a link to the internet failing is not this book's fault to catch.

Usage: check-links.py <book-output-dir>
"""

import re
import sys
from pathlib import Path


def main() -> int:
    if len(sys.argv) != 2:
        print(f"usage: {sys.argv[0]} <book-output-dir>", file=sys.stderr)
        return 2

    book = Path(sys.argv[1])
    html_files = sorted(book.glob("*.html"))
    if not html_files:
        print(f"no .html files found in {book}", file=sys.stderr)
        return 2

    known = {f.name for f in html_files}
    broken: list[tuple[str, str]] = []

    for page in html_files:
        html = page.read_text(encoding="utf-8")
        for href in re.findall(r'href="([^"]+)"', html):
            if href.startswith(("http://", "https://", "mailto:", "#")):
                continue
            target = href.split("#", 1)[0]
            if not target.endswith(".html"):
                continue
            if target not in known:
                broken.append((page.name, href))

    if broken:
        print(f"{len(broken)} broken internal link(s):", file=sys.stderr)
        for page, href in broken:
            print(f"  {page} -> {href}", file=sys.stderr)
        return 1

    print(f"{len(html_files)} pages, no broken internal links")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
