# Panel built with SwiftPM, without Xcode

The Panel is a Swift package built with `swift build` against the command line tools. A script wraps the binary into an `.app` bundle and signs it with `codesign`. Full Xcode is not installed on the development machine, and a compile of SwiftUI and `NSPanel` code succeeded without it. We give up SwiftUI previews, the visual layout debugger, and Instruments. Launching the bundled `.app` and showing the Panel is not yet verified, so it is the first task in the plan.
