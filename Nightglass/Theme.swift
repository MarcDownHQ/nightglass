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

    static func title(_ size: CGFloat, _ weight: Font.Weight = .bold) -> Font { .system(size: size, weight: weight).width(.condensed) }
    static func mono(_ size: CGFloat, _ weight: Font.Weight = .regular) -> Font { .system(size: size, weight: weight, design: .monospaced) }

    /// Stable neon for a name, so a channel keeps its colour between launches.
    static func neon(for name: String) -> Color {
        neons[abs(name.unicodeScalars.reduce(5381) { ($0 &* 33) &+ Int($1.value) }) % neons.count]
    }
}

extension Color {
    init(hex: UInt32) {
        self.init(red: Double(hex >> 16 & 0xFF) / 255, green: Double(hex >> 8 & 0xFF) / 255, blue: Double(hex & 0xFF) / 255)
    }
}

extension View {
    /// The neon tube: a tight core plus a wide soft halo.
    func neonGlow(_ color: Color, radius: CGFloat = 8) -> some View {
        shadow(color: color.opacity(0.9), radius: radius * 0.25).shadow(color: color.opacity(0.5), radius: radius)
    }
}

/// White text lit by a coloured neon halo.
struct NeonText: View {
    let text: String
    var size: CGFloat = 40
    var weight: Font.Weight = .heavy
    var color: Color = NG.magenta

    var body: some View {
        Text(text).font(NG.title(size, weight)).foregroundStyle(.white).neonGlow(color, radius: size * 0.35)
    }
}

/// Category filter: a neon outline that lights up when selected.
struct NeonTab: View {
    let title: String
    let selected: Bool
    var color: Color = NG.magenta
    let action: () -> Void
    @State private var hover = false

    var body: some View {
        Button(action: action) {
            Text(title).font(NG.title(16, .semibold)).lineLimit(1).fixedSize()
                .foregroundStyle(selected ? .white : hover ? NG.text : NG.dim)
                .padding(.horizontal, 14).padding(.vertical, 7)
                .background(selected ? color.opacity(0.18) : .clear, in: .capsule)
                .overlay(Capsule().strokeBorder(selected ? color : NG.text.opacity(hover ? 0.35 : 0.15), lineWidth: selected ? 1.5 : 1))
                .shadow(color: selected ? color.opacity(0.6) : .clear, radius: 8)
                .contentShape(.capsule)
        }
        .buttonStyle(.plain)
        .onHover { hover = $0 }
        .animation(.easeOut(duration: 0.15), value: hover)
        .animation(.spring(duration: 0.3), value: selected)
        .accessibilityAddTraits(selected ? .isSelected : [])
    }
}

/// Horizontal row of filters.
struct TabRow: View {
    let items: [String]
    @Binding var selection: String

    var body: some View {
        ScrollView(.horizontal) {
            HStack(spacing: 8) {
                ForEach(items, id: \.self) { c in NeonTab(title: c, selected: c == selection) { selection = c } }
            }
            .padding(.vertical, 10).padding(.horizontal, 4)
        }
        .scrollIndicators(.hidden)
    }
}

/// Dark glass panel with a faint cyan edge.
struct HUDPanel<Content: View>: View {
    var caption: String? = nil
    var trailing: String? = nil
    @ViewBuilder var content: Content
    @Environment(\.accessibilityReduceTransparency) private var opaque

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            if let caption {
                HStack {
                    Text(caption)
                    Spacer()
                    if let trailing { Text(trailing).foregroundStyle(NG.dim) }
                }
                .font(NG.title(15, .semibold)).foregroundStyle(NG.text)
                .padding(.horizontal, 16).padding(.vertical, 12)
                Rectangle().fill(NG.cyan.opacity(0.14)).frame(height: 1)
            }
            content
        }
        .background(opaque ? AnyShapeStyle(NG.ink) : AnyShapeStyle(NG.ink.opacity(0.7)), in: .rect(cornerRadius: 14))
        .overlay(RoundedRectangle(cornerRadius: 14).strokeBorder(NG.cyan.opacity(0.2), lineWidth: 1))
        .clipShape(.rect(cornerRadius: 14))
    }
}

/// Five bars; turn magenta when the stream degrades.
struct SignalBars: View {
    let level: Int
    var body: some View {
        let c = level <= 2 ? NG.magenta : NG.cyan
        HStack(alignment: .bottom, spacing: 2) {
            ForEach(1...5, id: \.self) { i in
                Rectangle().fill(c).frame(width: 3, height: CGFloat(i) * 2.8).opacity(i <= level ? 1 : 0.18)
            }
        }
        .accessibilityElement().accessibilityLabel("Signal \(level) sur 5")
    }
}

struct Tag: View {
    let text: String
    var color: Color = NG.acid
    var body: some View {
        Text(text).font(NG.mono(10, .medium)).foregroundStyle(color)
            .padding(.horizontal, 6).padding(.vertical, 2)
            .overlay(RoundedRectangle(cornerRadius: 4).stroke(color.opacity(0.5), lineWidth: 1))
    }
}

struct Background: View {
    var body: some View {
        LinearGradient(colors: [NG.ink, NG.night], startPoint: .top, endPoint: .bottom).ignoresSafeArea()
    }
}
