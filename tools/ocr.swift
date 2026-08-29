// tools/ocr.swift
//
// OCR a screenshot/image using Apple's Vision framework (macOS, no extra install).
// Usage:  swift tools/ocr.swift <image-path>
//
// Prints one line per recognized text observation as:  <y_top>\t<x_left>\t<text>
// (normalized coordinates, top-to-bottom then left-to-right) so tabular layout can be
// reconstructed. See docs/design.md §9.
//
// Tip: macOS screenshot filenames contain a narrow no-break space (U+202F); match
// them with a shell glob, e.g.  cp ~/Desktop/Screenshot\ 2026-07-06*.png /tmp/x.png

import Foundation
import Vision
import AppKit

let args = CommandLine.arguments
guard args.count > 1 else {
    FileHandle.standardError.write("usage: ocr <image>\n".data(using: .utf8)!); exit(1)
}
let path = args[1]
guard let img = NSImage(contentsOfFile: path),
      let tiff = img.tiffRepresentation,
      let bitmap = NSBitmapImageRep(data: tiff),
      let cg = bitmap.cgImage else {
    FileHandle.standardError.write("failed to load image\n".data(using: .utf8)!); exit(1)
}

let request = VNRecognizeTextRequest()
request.recognitionLevel = .accurate
request.usesLanguageCorrection = false
request.recognitionLanguages = ["en-US"]

let handler = VNImageRequestHandler(cgImage: cg, options: [:])
try handler.perform([request])

guard let results = request.results else { print("no results"); exit(0) }

struct Line { let x: Double; let y: Double; let text: String }
var lines: [Line] = []
for obs in results {
    guard let top = obs.topCandidates(1).first else { continue }
    let bb = obs.boundingBox // normalized, origin bottom-left
    let yTop = 1.0 - Double(bb.origin.y + bb.size.height)
    lines.append(Line(x: Double(bb.origin.x), y: yTop, text: top.string))
}
lines.sort { a, b in
    if abs(a.y - b.y) > 0.012 { return a.y < b.y }
    return a.x < b.x
}
for l in lines {
    print(String(format: "%.3f\t%.3f\t%@", l.y, l.x, l.text))
}
