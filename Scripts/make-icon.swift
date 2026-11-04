// Renders the Nightglass app icon at 1024×1024; the smaller sizes come from sips (see README).
//   swift Scripts/make-icon.swift
import AppKit
import SwiftUI

let ink = Color(red: 5 / 255, green: 1 / 255, blue: 15 / 255)
let night = Color(red: 26 / 255, green: 4 / 255, blue: 51 / 255)
let magenta = Color(red: 1, green: 42 / 255, blue: 109 / 255)
let cyan = Color(red: 5 / 255, green: 217 / 255, blue: 232 / 255)

/// A neon tube: wide soft halo, tighter glow, coloured tube, white-hot core.
func tube(_ g: inout GraphicsContext, _ path: Path, _ color: Color, width: CGFloat) {
    let style = StrokeStyle(lineWidth: width, lineCap: .round, lineJoin: .round)
    for (blur, alpha, w) in [(width * 2.2, 0.55, width * 1.6), (width * 0.9, 0.8, width * 1.15)] {
        g.drawLayer { l in
            l.addFilter(.blur(radius: blur))
            l.stroke(path, with: .color(color.opacity(alpha)), style: StrokeStyle(lineWidth: w, lineCap: .round, lineJoin: .round))
        }
    }
    g.stroke(path, with: .color(color), style: style)
    g.stroke(path, with: .color(.white.opacity(0.85)), style: StrokeStyle(lineWidth: width * 0.32, lineCap: .round, lineJoin: .round))
}

struct Icon: View {
    var body: some View {
        Canvas { g, size in
            let s = size.width
            g.fill(Path(CGRect(origin: .zero, size: size)),
                   with: .radialGradient(Gradient(colors: [night, ink]), center: CGPoint(x: s / 2, y: s * 0.45), startRadius: 0, endRadius: s * 0.75))
            // Screen.
            let screen = CGRect(x: s * 0.17, y: s * 0.22, width: s * 0.66, height: s * 0.5)
            tube(&g, Path(roundedRect: screen, cornerRadius: s * 0.09), cyan, width: s * 0.03)
            // Stand.
            var stand = Path()
            stand.move(to: CGPoint(x: s * 0.4, y: s * 0.8)); stand.addLine(to: CGPoint(x: s * 0.6, y: s * 0.8))
            tube(&g, stand, cyan, width: s * 0.03)
            // N as one continuous tube.
            var n = Path()
            n.move(to: CGPoint(x: s * 0.36, y: s * 0.62))
            n.addLine(to: CGPoint(x: s * 0.36, y: s * 0.32))
            n.addLine(to: CGPoint(x: s * 0.64, y: s * 0.62))
            n.addLine(to: CGPoint(x: s * 0.64, y: s * 0.32))
            tube(&g, n, magenta, width: s * 0.045)
        }
        .frame(width: 1024, height: 1024)
    }
}

@MainActor func render() throws {
    let renderer = ImageRenderer(content: Icon())
    renderer.scale = 1
    guard let cg = renderer.cgImage else { fatalError("render failed") }
    let rep = NSBitmapImageRep(cgImage: cg)
    let dir = URL(filePath: "Nightglass/Assets.xcassets/AppIcon.appiconset")
    try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
    try rep.representation(using: .png, properties: [:])!.write(to: dir.appending(path: "icon_512@2x.png"))
}

try MainActor.assumeIsolated { try render() }
