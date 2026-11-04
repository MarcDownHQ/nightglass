import NightglassCore
import SwiftUI

/// EPG grid: programmes are horizontal neon tubes, "now" is a cyan laser.
struct GuideView: View {
    @Environment(Store.self) private var store
    @Environment(Nav.self) private var nav

    static let perMinute: CGFloat = 5
    static let rowHeight: CGFloat = 58
    static let span: TimeInterval = 8 * 3600

    var body: some View {
        let start = Self.gridStart()
        let rows = store.channels.filter { !store.programmes(for: $0.channel).isEmpty }
        HUDPanel(caption: "GUIDE · \(rows.count) CHANNELS", trailing: Date.now.formatted(date: .abbreviated, time: .omitted).uppercased()) {
            if rows.isEmpty {
                Text("Aucun guide des programmes pour l'instant. Ajoute une URL XMLTV à ta source, ou attends la fin de la synchronisation.")
                    .font(NG.mono(12)).foregroundStyle(NG.dim).padding(40).frame(maxWidth: .infinity, maxHeight: .infinity)
            } else {
                ScrollView(.vertical) {
                    HStack(alignment: .top, spacing: 0) {
                        LazyVStack(alignment: .leading, spacing: 0, pinnedViews: []) {
                            Color.clear.frame(height: 30)
                            ForEach(rows) { item in
                                HStack(spacing: 10) {
                                    ChannelLogo(channel: item.channel, color: NG.neon(for: item.channel.name), size: 34)
                                    Text(displayName(item.channel.name).uppercased()).font(NG.title(16)).kerning(1).lineLimit(1)
                                }
                                .padding(.horizontal, 14).frame(width: 220, height: Self.rowHeight, alignment: .leading)
                                .overlay(alignment: .bottom) { Rectangle().fill(NG.cyan.opacity(0.08)).frame(height: 1) }
                            }
                        }
                        .frame(width: 220)
                        ScrollView(.horizontal) {
                            LazyVStack(alignment: .leading, spacing: 0) {
                                TimeRuler(start: start)
                                ForEach(rows) { item in GuideRow(item: item, start: start) }
                            }
                            .overlay(alignment: .topLeading) { NowLaser(start: start) }
                        }
                        .scrollIndicators(.hidden)
                        .defaultScrollAnchor(.leading)
                    }
                }
            }
        }
        .padding(.horizontal, 28).padding(.vertical, 16)
    }

    /// Half an hour before now, on the half hour.
    static func gridStart() -> Date {
        let t = Date.now.addingTimeInterval(-1800).timeIntervalSinceReferenceDate
        return Date(timeIntervalSinceReferenceDate: (t / 1800).rounded(.down) * 1800)
    }

    static func x(_ date: Date, from start: Date) -> CGFloat { CGFloat(date.timeIntervalSince(start) / 60) * perMinute }
}

struct TimeRuler: View {
    let start: Date
    var body: some View {
        ZStack(alignment: .leading) {
            ForEach(0..<Int(GuideView.span / 1800), id: \.self) { i in
                let d = start.addingTimeInterval(Double(i) * 1800)
                Text(hm(d)).font(NG.mono(10)).kerning(2).foregroundStyle(NG.cyan.opacity(0.7))
                    .offset(x: GuideView.x(d, from: start) + 6)
            }
        }
        .frame(width: GuideView.x(start.addingTimeInterval(GuideView.span), from: start), height: 30, alignment: .leading)
    }
}

struct GuideRow: View {
    @Environment(Store.self) private var store
    @Environment(Nav.self) private var nav
    let item: LiveItem
    let start: Date

    var body: some View {
        let end = start.addingTimeInterval(GuideView.span)
        let color = NG.neon(for: item.channel.name)
        ZStack(alignment: .leading) {
            ForEach(store.programmes(for: item.channel).filter { $0.end > start && $0.start < end }, id: \.self) { p in
                let x0 = max(0, GuideView.x(p.start, from: start)), x1 = GuideView.x(min(p.end, end), from: start)
                let live = p.start <= .now && p.end > .now
                Button { nav.focusChannel = item.id; nav.section = .live } label: {
                    VStack(alignment: .leading, spacing: 2) {
                        Text(p.title.uppercased()).font(NG.title(14, .semibold)).kerning(0.5).lineLimit(1)
                        Text("\(hm(p.start))–\(hm(p.end))").font(NG.mono(9)).foregroundStyle(NG.dim)
                    }
                    .padding(.horizontal, 10).frame(width: max(x1 - x0 - 4, 4), height: GuideView.rowHeight - 12, alignment: .leading)
                    .background(live ? color.opacity(0.18) : NG.ink.opacity(0.5), in: .rect(cornerRadius: 6))
                    .overlay(RoundedRectangle(cornerRadius: 6).strokeBorder(live ? color : NG.text.opacity(0.15), lineWidth: live ? 1.5 : 1))
                    .shadow(color: live ? color.opacity(0.5) : .clear, radius: 8)
                }
                .buttonStyle(.plain)
                .help(p.desc ?? p.title)
                .offset(x: x0 + 2)
            }
        }
        .frame(width: GuideView.x(end, from: start), height: GuideView.rowHeight, alignment: .leading)
        .overlay(alignment: .bottom) { Rectangle().fill(NG.cyan.opacity(0.08)).frame(height: 1) }
    }
}

struct NowLaser: View {
    let start: Date
    var body: some View {
        TimelineView(.periodic(from: .now, by: 30)) { ctx in
            Rectangle().fill(NG.cyan).frame(width: 2).neonGlow(NG.cyan, radius: 6)
                .offset(x: GuideView.x(ctx.date, from: start))
        }
        .allowsHitTesting(false)
    }
}
