import SwiftUI

enum NG {
    static let ink = Color(hex: 0x05010F)
    static let night = Color(hex: 0x1A0433)
    static let magenta = Color(hex: 0xFF2A6D)
    static let cyan = Color(hex: 0x05D9E8)
    static let acid = Color(hex: 0xF9F002)
    static let text = Color(hex: 0xF3EEFF)
    static let dim = text.opacity(0.55)
    static let neons = [magenta, cyan, acid]

    static func title(_ size: CGFloat, _ weight: Font.Weight = .bold) -> Font { .system(size: size, weight: weight).width(.compressed) }
    static func mono(_ size: CGFloat, _ weight: Font.Weight = .regular) -> Font { .system(size: size, weight: weight, design: .monospaced) }

    /// Stable neon for a name, so a category keeps its colour between launches.
    static func neon(for name: String) -> Color { neons[abs(stableHash(name)) % neons.count] }

    static func stableHash(_ s: String) -> Int { s.unicodeScalars.reduce(5381) { ($0 &* 33) &+ Int($1.value) } }

    /// Decorative katakana derived from a name. Never carries information.
    static func glyphs(for name: String) -> String {
        let kana = Array("アイウエオカキクケコサシスセソタチツテトナニヌネノハヒフヘホマミムメモヤユヨラリルレロワン")
        var h = abs(stableHash(name))
        return String((0..<min(4, max(2, name.count / 2))).map { _ in defer { h /= kana.count }; return kana[h % kana.count] })
    }
}

extension Color {
    init(hex: UInt32) {
        self.init(red: Double(hex >> 16 & 0xFF) / 255, green: Double(hex >> 8 & 0xFF) / 255, blue: Double(hex & 0xFF) / 255)
    }
}

/// Title with chromatic aberration: magenta left, cyan right, magenta halo.
struct ChromaText: View {
    let text: String
    var size: CGFloat = 40
    var weight: Font.Weight = .heavy

    var body: some View {
        let t = Text(text).font(NG.title(size, weight)).kerning(size * 0.03)
        ZStack {
            t.foregroundStyle(NG.magenta.opacity(0.9)).offset(x: -size * 0.04)
            t.foregroundStyle(NG.cyan.opacity(0.85)).offset(x: size * 0.04)
            t.foregroundStyle(.white)
        }
        .shadow(color: NG.magenta.opacity(0.45), radius: size * 0.3)
        .accessibilityElement().accessibilityLabel(text)
    }
}

/// Glow stack used by every neon element.
extension View {
    func neonGlow(_ color: Color, radius: CGFloat = 8) -> some View {
        shadow(color: color, radius: radius * 0.3).shadow(color: color.opacity(0.6), radius: radius).shadow(color: color.opacity(0.25), radius: radius * 3)
    }
}

/// The tube flickers on hover (2–3 dropouts) then holds, like a failing neon. Still with Reduce Motion.
struct NeonFlicker: ViewModifier {
    let trigger: Int
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    func body(content: Content) -> some View {
        if reduceMotion { content } else {
            content.keyframeAnimator(initialValue: 1.0, trigger: trigger) { view, opacity in view.opacity(opacity) } keyframes: { _ in
                KeyframeTrack {
                    CubicKeyframe(0.15, duration: 0.04); CubicKeyframe(1, duration: 0.06)
                    CubicKeyframe(1, duration: 0.08); CubicKeyframe(0.35, duration: 0.04); CubicKeyframe(1, duration: 0.05)
                }
            }
        }
    }
}

/// Vertical neon sign: a category, a filter, a section.
struct NeonSign: View {
    let title: String
    let selected: Bool
    var color: Color? = nil
    let action: () -> Void
    @State private var hover = false
    @State private var flicks = 0

    var body: some View {
        let c = color ?? NG.neon(for: title)
        let letters = Array(title.uppercased().prefix(9)) + (title.count > 9 ? ["…"] : [])
        Button(action: action) {
            HStack(alignment: .top, spacing: 4) {
                VStack(spacing: -2) {
                    ForEach(Array(letters.enumerated()), id: \.offset) { Text(String($0.element)) }
                }
                .font(NG.title(17, .heavy)).foregroundStyle(.white).neonGlow(c, radius: 6)
                VStack(spacing: 0) {
                    ForEach(Array(NG.glyphs(for: title).enumerated()), id: \.offset) { Text(String($0.element)) }
                }
                .font(.custom("Hiragino Sans", size: 10).weight(.semibold)).foregroundStyle(c.opacity(0.85))
            }
            .padding(.horizontal, 7).padding(.vertical, 10)
            .background(NG.ink.opacity(0.6), in: .rect(cornerRadius: 7))
            .overlay(RoundedRectangle(cornerRadius: 7).strokeBorder(c, lineWidth: 2).neonGlow(c, radius: 7))
            .opacity(selected ? 1 : hover ? 0.9 : 0.5)
            .saturation(selected || hover ? 1 : 0.7)
            .offset(y: selected ? 8 : 0)
            .modifier(NeonFlicker(trigger: flicks))
        }
        .buttonStyle(.plain)
        .onHover { hover = $0; if $0 { flicks += 1; Sound.buzz() } }
        .animation(.spring(duration: 0.35, bounce: 0.3), value: selected)
        .accessibilityLabel(title)
        .accessibilityAddTraits(selected ? .isSelected : [])
    }
}

/// Dark glass panel with 45° cut corners and a cyan hairline.
struct HUDPanel<Content: View>: View {
    var caption: String? = nil
    var trailing: String? = nil
    @ViewBuilder var content: Content
    @Environment(\.accessibilityReduceTransparency) private var opaque

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            if let caption {
                HStack {
                    Text("// " + caption)
                    Spacer()
                    if let trailing { Text(trailing) }
                }
                .font(NG.mono(10)).kerning(3).foregroundStyle(NG.cyan)
                .padding(.horizontal, 16).padding(.vertical, 11)
                Rectangle().fill(NG.cyan.opacity(0.14)).frame(height: 1)
            }
            content
        }
        .background { CutCorners().fill(opaque ? AnyShapeStyle(NG.ink) : AnyShapeStyle(NG.ink.opacity(0.62))) }
        .background { if !opaque { CutCorners().fill(.ultraThinMaterial).environment(\.colorScheme, .dark) } }
        .overlay { CutCorners().stroke(NG.cyan.opacity(0.22), lineWidth: 1) }
        .clipShape(CutCorners())
    }
}

struct CutCorners: Shape {
    var cut: CGFloat = 14
    func path(in r: CGRect) -> Path {
        Path { p in
            p.move(to: CGPoint(x: r.minX + cut, y: r.minY))
            p.addLine(to: CGPoint(x: r.maxX, y: r.minY))
            p.addLine(to: CGPoint(x: r.maxX, y: r.maxY - cut))
            p.addLine(to: CGPoint(x: r.maxX - cut, y: r.maxY))
            p.addLine(to: CGPoint(x: r.minX, y: r.maxY))
            p.addLine(to: CGPoint(x: r.minX, y: r.minY + cut))
            p.closeSubpath()
        }
    }
}

/// Five bars; turn magenta when the stream degrades.
struct SignalBars: View {
    let level: Int
    var body: some View {
        let c = level <= 2 ? NG.magenta : NG.cyan
        HStack(alignment: .bottom, spacing: 2) {
            ForEach(1...5, id: \.self) { i in
                Rectangle().fill(c).frame(width: 3, height: CGFloat(i) * 2.8)
                    .opacity(i <= level ? 1 : 0.18)
                    .shadow(color: i <= level ? c : .clear, radius: 3)
            }
        }
        .accessibilityElement().accessibilityLabel("Signal \(level) sur 5")
    }
}

struct Tag: View {
    let text: String
    var color: Color = NG.acid
    var body: some View {
        Text(text).font(NG.mono(10)).kerning(1.5).foregroundStyle(color)
            .padding(.horizontal, 6).padding(.vertical, 2)
            .overlay(Rectangle().stroke(color.opacity(0.45), lineWidth: 1))
    }
}

/// Ink background with slow neon bokeh tinted by the current content.
struct BokehBackground: View {
    var tint: Color = NG.magenta
    var paused = false
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    var body: some View {
        TimelineView(.animation(minimumInterval: 1 / 30, paused: paused || reduceMotion)) { ctx in
            let t = ctx.date.timeIntervalSinceReferenceDate * 0.05
            Canvas { g, size in
                g.fill(Path(CGRect(origin: .zero, size: size)),
                       with: .linearGradient(Gradient(colors: [NG.ink, NG.night]), startPoint: .zero, endPoint: CGPoint(x: 0, y: size.height)))
                g.addFilter(.blur(radius: 90))
                let blobs: [(Color, Double, Double, Double)] = [(tint, 0.2, 0.25, 0.42), (NG.cyan, 0.8, 0.7, 0.35), (NG.magenta, 0.65, 0.15, 0.3), (tint, 0.3, 0.85, 0.3)]
                for (i, b) in blobs.enumerated() {
                    let x = (b.1 + 0.06 * sin(t * (1 + Double(i) * 0.3))) * size.width
                    let y = (b.2 + 0.06 * cos(t * (0.8 + Double(i) * 0.2))) * size.height
                    let r = b.3 * min(size.width, size.height)
                    g.fill(Path(ellipseIn: CGRect(x: x - r, y: y - r, width: r * 2, height: r * 2)), with: .color(b.0.opacity(0.22)))
                }
            }
        }
        .ignoresSafeArea()
        .animation(.easeInOut(duration: 1.2), value: tint)
    }
}

/// Scrolling monospace ticker.
struct Wire: View {
    struct Item: Identifiable { let id: String; let name: String; let text: String; let color: Color }
    let items: [Item]
    let onTap: (String) -> Void
    @State private var width: CGFloat = 1
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    var body: some View {
        HStack(spacing: 0) {
            Text("● NG WIRE").font(NG.mono(12, .bold)).kerning(1.5).foregroundStyle(NG.ink)
                .padding(.horizontal, 16).frame(maxHeight: .infinity).background(NG.magenta).neonGlow(NG.magenta, radius: 6)
            GeometryReader { _ in
                TimelineView(.animation(paused: reduceMotion || items.isEmpty)) { ctx in
                    let offset = CGFloat(ctx.date.timeIntervalSinceReferenceDate * 40).truncatingRemainder(dividingBy: max(width, 1))
                    HStack(spacing: 0) { row; row }.offset(x: -offset)
                }
            }
            .clipped()
        }
        .frame(height: 34)
        .background(NG.ink.opacity(0.8))
        .overlay(alignment: .top) { Rectangle().fill(NG.magenta.opacity(0.4)).frame(height: 1) }
    }

    var row: some View {
        HStack(spacing: 0) {
            ForEach(items) { item in
                Button { onTap(item.id) } label: {
                    (Text(item.name).foregroundStyle(item.color).bold() + Text(" · " + item.text).foregroundStyle(NG.text))
                        .font(NG.mono(12)).kerning(1.4)
                }
                .buttonStyle(.plain)
                Text("◆").font(NG.mono(12)).foregroundStyle(NG.acid).padding(.horizontal, 28)
            }
        }
        .fixedSize()
        .onGeometryChange(for: CGFloat.self) { $0.size.width } action: { width = $0 }
    }
}
