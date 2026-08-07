import CoreGraphics
import Foundation
import ImageIO
import PhotosUI
import Tauri
import UIKit
import UniformTypeIdentifiers
import VisionKit

private struct ScanArgs: Decodable {
  let outputPath: String
}

/** Presents VisionKit's reviewable multi-page scanner and streams its page
 * images into one app-owned PDF instead of serializing image data through IPC. */
final class DocumentScannerPlugin: Plugin, VNDocumentCameraViewControllerDelegate,
  PHPickerViewControllerDelegate {
  private var pendingInvoke: Invoke?
  private var outputURL: URL?

  @objc func availability(_ invoke: Invoke) {
    invoke.resolve([
      "supported": VNDocumentCameraViewController.isSupported,
      "photoImportSupported": true
    ])
  }

  /** Presents Apple's privacy-preserving photo picker. Selected files are
   * copied before its temporary grants expire, then converted off the UI thread. */
  @objc func importImages(_ invoke: Invoke) {
    DispatchQueue.main.async {
      guard self.pendingInvoke == nil else {
        invoke.reject("Another document import is already in progress")
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
          invoke.reject("Unable to present the photo picker")
          return
        }

        self.pendingInvoke = invoke
        self.outputURL = outputURL
        var configuration = PHPickerConfiguration()
        configuration.filter = .images
        configuration.selectionLimit = ImageImportLimits.maximumImages
        let picker = PHPickerViewController(configuration: configuration)
        picker.delegate = self
        presenter.present(picker, animated: true)
      } catch {
        invoke.reject(error.localizedDescription)
      }
    }
  }

  func picker(_ picker: PHPickerViewController, didFinishPicking results: [PHPickerResult]) {
    picker.dismiss(animated: true)
    guard let invoke = pendingInvoke, let outputURL else {
      clearPendingOperation()
      return
    }
    guard !results.isEmpty else {
      invoke.reject("Photo import cancelled")
      clearPendingOperation()
      return
    }

    let sessionURL = outputURL.deletingLastPathComponent()
      .appendingPathComponent(".photo-import-\(UUID().uuidString)", isDirectory: true)
    do {
      try FileManager.default.createDirectory(
        at: sessionURL,
        withIntermediateDirectories: true
      )
    } catch {
      invoke.reject(error.localizedDescription)
      clearPendingOperation()
      return
    }

    copySelectedImages(results, index: 0, sessionURL: sessionURL, copied: []) { result in
      switch result {
      case .failure(let error):
        try? FileManager.default.removeItem(at: sessionURL)
        DispatchQueue.main.async {
          invoke.reject(error.localizedDescription)
          self.clearPendingOperation()
        }
      case .success(let imageURLs):
        DispatchQueue.global(qos: .userInitiated).async {
          do {
            try self.writeImagePDF(imageURLs, to: outputURL)
            try? FileManager.default.removeItem(at: sessionURL)
            DispatchQueue.main.async {
              invoke.resolve([
                "outputPath": outputURL.path,
                "pageCount": imageURLs.count
              ])
              self.clearPendingOperation()
            }
          } catch {
            try? FileManager.default.removeItem(at: outputURL)
            try? FileManager.default.removeItem(at: sessionURL)
            DispatchQueue.main.async {
              invoke.reject(error.localizedDescription)
              self.clearPendingOperation()
            }
          }
        }
      }
    }
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
    guard scan.pageCount <= DocumentOutputLimits.maximumPages else {
      invoke.reject(ScannerError.tooManyPages.errorDescription ?? "Document scan is too large")
      finish(controller)
      return
    }

    // VisionKit calls its delegate on the main thread. Dismiss immediately,
    // then assemble the potentially large PDF in the background so the Tauri
    // WebView can paint its processing state and remain responsive.
    controller.dismiss(animated: true) {
      DispatchQueue.global(qos: .userInitiated).async {
        do {
          try self.writePDF(scan, to: outputURL)
          DispatchQueue.main.async {
            invoke.resolve([
              "outputPath": outputURL.path,
              "pageCount": scan.pageCount
            ])
            self.clearPendingOperation()
          }
        } catch {
          try? FileManager.default.removeItem(at: outputURL)
          DispatchQueue.main.async {
            invoke.reject(error.localizedDescription)
            self.clearPendingOperation()
          }
        }
      }
    }
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
    guard scan.pageCount <= DocumentOutputLimits.maximumPages else {
      throw ScannerError.tooManyPages
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
      try ensureOutputWithinLimit(outputURL)
    }
    context.closePDF()
    try ensureOutputWithinLimit(outputURL)
  }

  /** Loads one picker result at a time and copies it immediately because the
   * provider's temporary URL is invalid after its completion handler returns. */
  private func copySelectedImages(
    _ results: [PHPickerResult],
    index: Int,
    sessionURL: URL,
    copied: [URL],
    completion: @escaping (Result<[URL], Error>) -> Void
  ) {
    guard index < results.count else {
      completion(.success(copied))
      return
    }
    let provider = results[index].itemProvider
    guard provider.hasItemConformingToTypeIdentifier(UTType.image.identifier) else {
      completion(.failure(ScannerError.unsupportedImage(index + 1)))
      return
    }

    provider.loadFileRepresentation(forTypeIdentifier: UTType.image.identifier) { sourceURL, error in
      do {
        if let error { throw error }
        guard let sourceURL else { throw ScannerError.cannotReadPage(index + 1) }
        let attributes = try FileManager.default.attributesOfItem(atPath: sourceURL.path)
        let bytes = (attributes[.size] as? NSNumber)?.int64Value ?? 0
        guard bytes > 0 else { throw ScannerError.cannotReadPage(index + 1) }
        guard bytes <= ImageImportLimits.maximumSourceBytes else {
          throw ScannerError.imageTooLarge(index + 1)
        }

        let suffix = sourceURL.pathExtension.isEmpty ? "image" : sourceURL.pathExtension
        let destination = sessionURL.appendingPathComponent("page-\(index).\(suffix)")
        try FileManager.default.copyItem(at: sourceURL, to: destination)
        self.copySelectedImages(
          results,
          index: index + 1,
          sessionURL: sessionURL,
          copied: copied + [destination],
          completion: completion
        )
      } catch {
        completion(.failure(error))
      }
    }
  }

  /** Downsamples and draws one selected image per PDF page. ImageIO applies
   * orientation during thumbnail creation while capping decode memory. */
  private func writeImagePDF(_ imageURLs: [URL], to outputURL: URL) throws {
    guard !imageURLs.isEmpty else { throw ScannerError.emptyScan }
    guard imageURLs.count <= DocumentOutputLimits.maximumPages else {
      throw ScannerError.tooManyPages
    }
    guard let consumer = CGDataConsumer(url: outputURL as CFURL),
          let context = CGContext(consumer: consumer, mediaBox: nil, nil) else {
      throw ScannerError.cannotCreatePDF
    }

    for (index, imageURL) in imageURLs.enumerated() {
      try autoreleasepool {
        guard let source = CGImageSourceCreateWithURL(imageURL as CFURL, nil),
              let image = CGImageSourceCreateThumbnailAtIndex(source, 0, [
                kCGImageSourceCreateThumbnailFromImageAlways: true,
                kCGImageSourceCreateThumbnailWithTransform: true,
                kCGImageSourceThumbnailMaxPixelSize: ImageImportLimits.maximumPixelDimension
              ] as CFDictionary) else {
          throw ScannerError.cannotReadPage(index + 1)
        }
        let width = CGFloat(image.width) * 72 / 300
        let height = CGFloat(image.height) * 72 / 300
        let pageBounds = CGRect(x: 0, y: 0, width: width, height: height)
        let pageInfo = [kCGPDFContextMediaBox as String: pageBounds] as CFDictionary
        context.beginPDFPage(pageInfo)
        context.saveGState()
        context.translateBy(x: 0, y: height)
        context.scaleBy(x: 1, y: -1)
        context.draw(image, in: pageBounds)
        context.restoreGState()
        context.endPDFPage()
      }
      try ensureOutputWithinLimit(outputURL)
    }
    context.closePDF()
    try ensureOutputWithinLimit(outputURL)
  }

  /** Bounds app-owned output while Core Graphics streams pages. Rust checks the
   * completed file again because native plugin results still cross IPC. */
  private func ensureOutputWithinLimit(_ outputURL: URL) throws {
    // Core Graphics may not materialize its consumer file until the first
    // flush. The post-close check and Rust boundary still verify the result.
    guard FileManager.default.fileExists(atPath: outputURL.path) else { return }
    let attributes = try FileManager.default.attributesOfItem(atPath: outputURL.path)
    let bytes = (attributes[.size] as? NSNumber)?.int64Value ?? 0
    if bytes > DocumentOutputLimits.maximumBytes {
      throw ScannerError.outputTooLarge
    }
  }

  private func finish(_ controller: VNDocumentCameraViewController) {
    clearPendingOperation()
    controller.dismiss(animated: true)
  }

  private func clearPendingOperation() {
    pendingInvoke = nil
    outputURL = nil
  }
}

private enum ImageImportLimits {
  static let maximumImages = DocumentOutputLimits.maximumPages
  static let maximumSourceBytes: Int64 = 64 * 1024 * 1024
  static let maximumPixelDimension = 3000
}

private enum DocumentOutputLimits {
  // Keep native work bounded before the Rust importer independently enforces
  // its canonical PDF limit at the IPC boundary.
  static let maximumPages = 500
  static let maximumBytes: Int64 = 250 * 1024 * 1024
}

private enum ScannerError: LocalizedError {
  case emptyScan
  case cannotCreatePDF
  case cannotReadPage(Int)
  case imageTooLarge(Int)
  case outputTooLarge
  case tooManyPages
  case unsupportedImage(Int)

  var errorDescription: String? {
    switch self {
    case .emptyScan:
      return "The document scan has no pages"
    case .cannotCreatePDF:
      return "Unable to create the scanned PDF"
    case .cannotReadPage(let page):
      return "Unable to read scanned page \(page)"
    case .imageTooLarge(let page):
      return "Selected photo \(page) exceeds the 64 MB limit"
    case .outputTooLarge:
      return "The scanned document exceeds the 250 MB PDF limit"
    case .tooManyPages:
      return "The scanned document exceeds the 500-page limit"
    case .unsupportedImage(let page):
      return "Selected item \(page) is not a supported image"
    }
  }
}

@_cdecl("init_plugin_document_scanner")
func initPlugin() -> Plugin {
  DocumentScannerPlugin()
}
