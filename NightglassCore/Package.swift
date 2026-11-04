// swift-tools-version: 6.0
import PackageDescription

let package = Package(
    name: "NightglassCore",
    platforms: [.macOS(.v15)],
    products: [.library(name: "NightglassCore", targets: ["NightglassCore"])],
    targets: [
        .target(name: "NightglassCore"),
        .testTarget(name: "NightglassCoreTests", dependencies: ["NightglassCore"]),
    ]
)
