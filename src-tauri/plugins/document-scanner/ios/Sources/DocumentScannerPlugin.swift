import CoreGraphics
import Foundation
import Tauri
import UIKit
import VisionKit

private struct ScanArgs: Decodable {
  let outputPath: String
}

/** Presents VisionKit's reviewable multi-page scanner and streams its page
 * images into one app-owned PDF instead of serializing image data through IPC. */
final class DocumentScannerPlugin: Plugin, VNDocumentCameraViewControllerDelegate {
  private var pendingInvoke: Invoke?
  private var outputURL: URL?

  @objc func availability(_ invoke: Invoke) {
    invoke.resolve([
      "supported": VNDocumentCameraViewController.isSupported,
      "platform": "ios",
      "reason": VNDocumentCameraViewController.isSupported
        ? NSNull()
        : "Document scanning is not supported on this device"
    ])
  }

  @objc func scan(_ invoke: Invoke) {
    DispatchQueue.main.async {
      guard VNDocumentCameraViewController.isSupported else {
        invoke.reject("Document scanning is not supported on this device")
        return
      }
      guard self.pendingInvoke == nil else {
        invoke.reject("Another document scan is already in progress")
        return
      }

      do {
        let args = try invoke.parseArgs(ScanArgs.self)
        let outputURL = URL(fileURLWithPath: args.outputPath)
        try FileManager.default.createDirectory(
          at: outputURL.deletingLastPathComponent(),
          withIntermediateDirectories: true
        )

        guard let presenter = self.manager.viewController else {
          invoke.reject("Unable to present the document scanner")
          return
        }

        self.pendingInvoke = invoke
        self.outputURL = outputURL
        let scanner = VNDocumentCameraViewController()
        scanner.delegate = self
        presenter.present(scanner, animated: true)
      } catch {
        invoke.reject(error.localizedDescription)
      }
    }
  }

  func documentCameraViewController(
    _ controller: VNDocumentCameraViewController,
    didFinishWith scan: VNDocumentCameraScan
  ) {
    guard let invoke = pendingInvoke, let outputURL else {
      controller.dismiss(animated: true)
      return
    }

    do {
      try writePDF(scan, to: outputURL)
      invoke.resolve([
        "outputPath": outputURL.path,
        "pageCount": scan.pageCount
      ])
    } catch {
      try? FileManager.default.removeItem(at: outputURL)
      invoke.reject(error.localizedDescription)
    }
    finish(controller)
  }

  func documentCameraViewControllerDidCancel(_ controller: VNDocumentCameraViewController) {
    pendingInvoke?.reject("Document scan cancelled")
    finish(controller)
  }

  func documentCameraViewController(
    _ controller: VNDocumentCameraViewController,
    didFailWithError error: Error
  ) {
    pendingInvoke?.reject(error.localizedDescription)
    finish(controller)
  }

  /** Core Graphics writes each page as it is requested from VisionKit, avoiding
   * one additional full-document byte buffer in memory for large scans. */
  private func writePDF(_ scan: VNDocumentCameraScan, to outputURL: URL) throws {
    guard scan.pageCount > 0 else {
      throw ScannerError.emptyScan
    }
    guard let consumer = CGDataConsumer(url: outputURL as CFURL),
          let context = CGContext(consumer: consumer, mediaBox: nil, nil) else {
      throw ScannerError.cannotCreatePDF
    }

    for pageIndex in 0..<scan.pageCount {
      try autoreleasepool {
        let image = scan.imageOfPage(at: pageIndex)
        guard let cgImage = image.cgImage else {
          throw ScannerError.cannotReadPage(pageIndex + 1)
        }
        let pageBounds = CGRect(origin: .zero, size: image.size)
        let pageInfo = [kCGPDFContextMediaBox as String: pageBounds] as CFDictionary
        context.beginPDFPage(pageInfo)
        context.saveGState()
        context.translateBy(x: 0, y: pageBounds.height)
        context.scaleBy(x: 1, y: -1)
        context.draw(cgImage, in: pageBounds)
        context.restoreGState()
        context.endPDFPage()
      }
    }
    context.closePDF()
  }

  private func finish(_ controller: VNDocumentCameraViewController) {
    pendingInvoke = nil
    outputURL = nil
    controller.dismiss(animated: true)
  }
}

private enum ScannerError: LocalizedError {
  case emptyScan
  case cannotCreatePDF
  case cannotReadPage(Int)

  var errorDescription: String? {
    switch self {
    case .emptyScan:
      return "The document scan has no pages"
    case .cannotCreatePDF:
      return "Unable to create the scanned PDF"
    case .cannotReadPage(let page):
      return "Unable to read scanned page \(page)"
    }
  }
}

@_cdecl("init_plugin_document_scanner")
func initPlugin() -> Plugin {
  DocumentScannerPlugin()
}
