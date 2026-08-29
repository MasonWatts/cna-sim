#!/usr/bin/env bash
# tools/pdf-page.sh — render a single PDF page to a high-DPI PNG for OCR.
#
# Usage:  tools/pdf-page.sh <pdf-path> <page-number> [dpi] [out-prefix]
#   dpi         default 300 (raise to 450/600 for very dense charts)
#   out-prefix  default /tmp/pdf_page  (pdftoppm appends -<page>.png)
#
# Then OCR the result:  swift tools/ocr.swift <out>.png
#
# Rendering from the vector PDF is much cleaner than screenshots.
set -euo pipefail
pdf="${1:?usage: pdf-page.sh <pdf> <page> [dpi] [out-prefix]}"
page="${2:?missing page number}"
dpi="${3:-300}"
prefix="${4:-/tmp/pdf_page}"

pdftoppm -png -f "$page" -l "$page" -r "$dpi" "$pdf" "$prefix"
# pdftoppm zero-pads the page suffix to the width of the last page number.
ls -1 "${prefix}"-*"${page}".png 2>/dev/null | tail -1
