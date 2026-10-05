// swift-tools-version: 5.9
import PackageDescription

let package = Package(
    name: "Nook",
    platforms: [
        .macOS(.v14)
    ],
    products: [
        .executable(name: "Nook", targets: ["Nook"]),
        .library(name: "NookCore", targets: ["NookCore"]),
        .executable(name: "NookTests", targets: ["NookTests"])
    ],
    dependencies: [],
    targets: [
        .target(
            name: "NookCore",
            dependencies: [],
            path: "Sources/NookCore"
        ),
        .executableTarget(
            name: "Nook",
            dependencies: ["NookCore"],
            path: "Sources/Nook"
        ),
        .executableTarget(
            name: "NookTests",
            dependencies: ["NookCore"],
            path: "Tests/NookTests"
        )
    ]
)
