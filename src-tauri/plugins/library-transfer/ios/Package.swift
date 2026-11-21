// swift-tools-version:5.9

import PackageDescription

let package = Package(
  name: "tauri-plugin-library-transfer",
  platforms: [.iOS(.v14)],
  products: [
    .library(
      name: "tauri-plugin-library-transfer",
      type: .static,
      targets: ["tauri-plugin-library-transfer"]
    )
  ],
  dependencies: [
    .package(name: "Tauri", path: "../.tauri/tauri-api")
  ],
  targets: [
    .target(
      name: "tauri-plugin-library-transfer",
      dependencies: [.byName(name: "Tauri")],
      path: "Sources"
    )
  ]
)
