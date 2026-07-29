import Foundation

// Keeps the app target Swift-aware so Xcode links Swift runtime libraries
// needed by Tauri and plugin Swift objects inside libapp.a.
@objc final class SwiftRuntimeBridge: NSObject {}
