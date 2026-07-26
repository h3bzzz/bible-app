#!/usr/bin/env python3
"""Extracts scripture reference pairs from the '351 Old Testament Prophecies
Fulfilled in Jesus Christ' page at newtestamentchristians.com.

Emits messianic-prophecy-refs.tsv: one line per table row,
`OT_REF<TAB>FULFILMENT;FULFILMENT;...`, with book names normalised to USFM
codes. References remain in ENGLISH (KJV) versification exactly as published;
`tools/build-annotations` maps them into the canonical `org` scheme, which is
a versification judgement this scraper must not make.

Only the reference pairs are extracted. The page's descriptive prose is its
author's work and is deliberately not copied; which verse the New Testament
cites is a fact, and facts are not copyrightable (Feist v. Rural, 499 U.S.
340). Provenance is recorded in the layer's licence attribution.

Usage: extract_prophecy_refs.py <saved-page.html> > messianic-prophecy-refs.tsv
"""

import html
import re
import sys

# Book tokens as the page spells them -> USFM codes.
BOOKS = {
    "Gen": "GEN", "Ex": "EXO", "Lev": "LEV", "Num": "NUM", "Deut": "DEU",
    "Joshua": "JOS", "Ruth": "RUT", "1 Sam": "1SA", "2 Sam": "2SA",
    "2 Ki": "2KI", "1 Chr": "1CH", "Job": "JOB", "Psa": "PSA", "Prov": "PRO",
    "Song": "SNG", "Isa": "ISA", "Jer": "JER", "Ezek": "EZK", "Dan": "DAN",
    "Hos": "HOS", "Joel": "JOL", "Jonah": "JON", "Mic": "MIC", "Hag": "HAG",
    "Zech": "ZEC", "Mal": "MAL",
    "Matt": "MAT", "Mark": "MRK", "Luke": "LUK", "John": "JHN",
    "Act": "ACT", "Acts": "ACT", "Rom": "ROM", "1 Cor": "1CO", "2 Cor": "2CO",
    "Gal": "GAL", "Eph": "EPH", "Phil": "PHP", "Col": "COL", "1 Tim": "1TI",
    "Titus": "TIT", "Heb": "HEB", "1 Pet": "1PE", "2 Pet": "2PE",
    "1 John": "1JN", "Rev": "REV",
}

REF = re.compile(
    r"^((?:[123]\s+)?[A-Za-z]+)\.?\s+(\d+)(?::(\d+))?"  # book chapter[:verse]
    r"(?:-(?:(\d+):)?(\d+))?[a-f]?$"                    # [-[chapter:]verse][subverse]
)


def parse_ref(token, book, chapter):
    """One reference token -> (canonical string, book, chapter).

    `book`/`chapter` carry context for continuation tokens: in
    "Matt 3:1-3; 6" the "6" is Matthew 3:6, and in "19:11, 15" the "15" is
    verse 15 of the chapter already named.
    """
    token = re.sub(r"[a-f]$", "", token.strip())

    m = re.match(r"^(\d+):(\d+)(?:-(?:(\d+):)?(\d+))?$", token)  # C:V[-[C:]V]
    if m and book:
        c1, v1, c2, v2 = m.groups()
        return fmt(book, c1, v1, c2 or c1, v2), book, c1
    m = re.match(r"^(\d+)(?:-(\d+))?$", token)  # V[-V] continuation
    if m and book and chapter:
        v1, v2 = m.groups()
        return fmt(book, chapter, v1, chapter, v2), book, chapter

    m = REF.match(token)
    if not m:
        raise ValueError(f"unparseable reference {token!r}")
    name, c1, v1, c2, v2 = m.groups()
    code = BOOKS[name]
    if v1 is None:
        # Chapter-only reference: the whole chapter.
        return f"{code} {c1}", code, c1
    return fmt(code, c1, v1, c2 or c1, v2), code, c1


def fmt(book, c1, v1, c2, v2):
    if v2 is None or (c1 == c2 and v1 == v2):
        return f"{book} {c1}:{v1}"
    if c1 == c2:
        return f"{book} {c1}:{v1}-{v2}"
    return f"{book} {c1}:{v1}-{c2}:{v2}"


def refs_of(field):
    """A fulfilment cell -> list of canonical reference strings."""
    out, book, chapter = [], None, None
    for part in field.split(";"):
        for token in part.split(","):
            token = token.strip()
            if not token:
                continue
            ref, book, chapter = parse_ref(token, book, chapter)
            out.append(ref)
    return out


def main(path):
    src = open(path, encoding="utf-8").read()
    rows = re.findall(r"<tr[^>]*>(.*?)</tr>", src, re.S)[1:]  # skip header
    count = 0
    for row in rows:
        cells = [
            html.unescape(re.sub(r"<[^>]+>", "", c)).strip()
            for c in re.findall(r"<t[dh][^>]*>(.*?)</t[dh]>", row, re.S)
        ]
        if len(cells) != 3:
            raise ValueError(f"expected 3 cells, got {cells!r}")
        ot_raw = re.sub(r"^\d+\.\s*", "", cells[0]).strip()
        ot, _, _ = parse_ref(ot_raw, None, None)
        fulfilments = refs_of(cells[2])
        if not fulfilments:
            raise ValueError(f"{ot_raw}: no fulfilment references")
        print(f"{ot}\t{';'.join(fulfilments)}")
        count += 1
    print(f"extracted {count} rows", file=sys.stderr)


if __name__ == "__main__":
    main(sys.argv[1])
