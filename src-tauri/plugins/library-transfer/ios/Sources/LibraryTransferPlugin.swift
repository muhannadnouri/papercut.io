import Tauri
import UIKit

private struct ObserveArgs: Decodable { let onState: Channel }

@objc(LibraryTransferPlugin)
final class LibraryTransferPlugin: Plugin {
  private var observers: [NSObjectProtocol] = []

  @objc func observe(_ invoke: Invoke) throws {
    let channel = try invoke.parseArgs(ObserveArgs.self).onState
    observers.forEach { NotificationCenter.default.removeObserver($0) }
    // Resigning active also happens during permission prompts; it is not backgrounding.
    observers = [
      NotificationCenter.default.addObserver(forName: UIApplication.didEnterBackgroundNotification, object: nil, queue: .main) { _ in
        try? channel.send(true)
      },
      NotificationCenter.default.addObserver(forName: UIApplication.willEnterForegroundNotification, object: nil, queue: .main) { _ in
        try? channel.send(false)
      }
    ]
    try channel.send(UIApplication.shared.applicationState == .background)
    invoke.resolve()
  }

  deinit { observers.forEach { NotificationCenter.default.removeObserver($0) } }
}

@_cdecl("init_plugin_library_transfer")
func initPlugin() -> Plugin { LibraryTransferPlugin() }
