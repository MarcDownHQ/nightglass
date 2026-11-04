import NightglassCore
import SwiftUI

/// EPG grid. A 3-hour window fitted to the width and paged with ←/→, so the only scroll is vertical
/// and the rows stay lazy (a horizontal ScrollView around them would build every row at once).
struct GuideView: View {
    @Environment(Store.self) private var store
    @State private var shift = 0
    @FocusState private var focused: Bool

    static let rowHeight: CGFloat = 58
    static let labelWidth: CGFloat = 220
    static let window: TimeInterval = 3 * 3600

    var body: some View {
        let start = Self.gridStart().addingTimeInterval(Double(shift) * 1800)
        let rows = store.channels.filter { !store.programmes(for: $0.channel).isEmpty }
        HUDPanel(caption: "Guide", trailing: start.formatted(date: .abbreviated, time: .omitted)) {
            if rows.isEmpty {
                Text("No programme guide yet. Add an XMLTV URL to a source, or wait for it to load.")
                    .font(NG.title(16, .regular)).foregroundStyle(NG.dim).padding(40).frame(maxWidth: .infinity, maxHeight: .infinity)
            } else {
                GeometryReader { g in
                    let perSecond = (g.size.width - Self.labelWidth) / Self.window
                    VStack(spacing: 0) {
                        HStack(spacing: 0) {
                            HStack(spacing: 6) {
                                pageButton("chevron.left", "Earlier (←)") { shift -= 2 }
                                Button("Now") { shift = 0 }.buttonStyle(.plain).font(NG.title(14, .semibold))
                                    .foregroundStyle(shift == 0 ? NG.dim : NG.cyan).disabled(shift == 0)
                                pageButton("chevron.right", "Later (→)") { shift += 2 }
                            }
                            .padding(.horizontal, 14).frame(width: Self.labelWidth, alignment: .leading)
                            TimeRuler(start: start, perSecond: perSecond)
                        }
                        .frame(height: 34)
                        Rectangle().fill(NG.cyan.opacity(0.1)).frame(height: 1)
                        ScrollView {
                            LazyVStack(spacing: 0) {
                                ForEach(rows) { item in GuideRow(item: item, start: start, perSecond: perSecond) }
                            }
                        }
                    }
                }
            }
        }
        .padding(.horizontal, 28).padding(.vertical, 16)
        .focusable().focusEffectDisabled().focused($focused)
        .onAppear { focused = true }
        .onKeyPress(.leftArrow) { shift -= 2; return .handled }
        .onKeyPress(.rightArrow) { shift += 2; return .handled }
    }

    func pageButton(_ icon: String, _ help: String, action: @escaping () -> Void) -> some View {
        Button(action: action) { Image(systemName: icon).font(.system(size: 13, weight: .semibold)).padding(6).contentShape(.rect) }
            .buttonStyle(.plain).foregroundStyle(NG.cyan).help(help)
    }

    /// Half an hour before now, on the half hour.
    static func gridStart() -> Date {
        let t = Date.now.addingTimeInterval(-1800).timeIntervalSinceReferenceDate
        return Date(timeIntervalSinceReferenceDate: (t / 1800).rounded(.down) * 1800)
    }
}

struct TimeRuler: View {
    let start: Date
    let perSecond: CGFloat
    var body: some View {
        ZStack(alignment: .leading) {
            ForEach(0..<Int(GuideView.window / 1800), id: \.self) { i in
                Text(hm(start.addingTimeInterval(Double(i) * 1800))).font(NG.mono(11)).foregroundStyle(NG.dim)
                    .offset(x: CGFloat(i) * 1800 * perSecond + 6)
            }
        }
        .frame(maxWidth: .infinity, alignment: .leading)
    }
}

struct GuideRow: View {
    @Environment(Store.self) private var store
    @Environment(Nav.self) private var nav
    let item: LiveItem
    let start: Date
    let perSecond: CGFloat

    var body: some View {
        let end = start.addingTimeInterval(GuideView.window)
        let color = NG.neon(for: item.channel.name)
        let now = Date.now
        HStack(spacing: 0) {
            HStack(spacing: 10) {
                ChannelLogo(channel: item.channel, color: color, size: 34)
                Text(displayName(item.channel.name)).font(NG.title(16, .semibold)).lineLimit(1)
            }
            .padding(.horizontal, 14).frame(width: GuideView.labelWidth, alignment: .leading)
            ZStack(alignment: .leading) {
                ForEach(store.programmes(for: item.channel).filter { $0.end > start && $0.start < end }, id: \.self) { p in
                    let x0 = max(0, p.start.timeIntervalSince(start)) * perSecond
                    let x1 = min(p.end, end).timeIntervalSince(start) * perSecond
                    let live = p.start <= now && p.end > now
                    Button { nav.focusChannel = item.id; nav.section = .live } label: {
                        VStack(alignment: .leading, spacing: 2) {
                            Text(p.title).font(NG.title(14, .semibold)).lineLimit(1)
                            Text("\(hm(p.start))–\(hm(p.end))").font(NG.mono(9)).foregroundStyle(NG.dim).lineLimit(1)
                        }
                        .padding(.horizontal, 10).frame(width: max(x1 - x0 - 4, 2), height: GuideView.rowHeight - 12, alignment: .leading)
                        .background(live ? color.opacity(0.2) : NG.ink.opacity(0.5), in: .rect(cornerRadius: 6))
                        .overlay(RoundedRectangle(cornerRadius: 6).strokeBorder(live ? color : NG.text.opacity(0.15), lineWidth: live ? 1.5 : 1))
                    }
                    .buttonStyle(.plain)
                    .help(p.desc ?? p.title)
                    .offset(x: x0 + 2)
                }
                if now > start && now < end {
                    Rectangle().fill(NG.cyan).frame(width: 2).offset(x: now.timeIntervalSince(start) * perSecond).allowsHitTesting(false)
                }
            }
            .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .leading)
            .clipped()
        }
        .frame(height: GuideView.rowHeight)
        .overlay(alignment: .bottom) { Rectangle().fill(NG.cyan.opacity(0.08)).frame(height: 1) }
    }
}
