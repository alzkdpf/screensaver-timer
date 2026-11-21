import CoreGraphics
import Foundation
import ImageIO
import UniformTypeIdentifiers

let destination = URL(fileURLWithPath: CommandLine.arguments[1], isDirectory: true)
let space = CGColorSpace(name: CGColorSpace.sRGB)!
let mint = CGColor(red: 0.47, green: 0.97, blue: 0.76, alpha: 1)
let pale = CGColor(red: 0.90, green: 1, blue: 0.97, alpha: 1)

func icon(size: Int) -> CGImage {
    let ctx = CGContext(data: nil, width: size, height: size, bitsPerComponent: 8,
                        bytesPerRow: 0, space: space,
                        bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue)!
    let scale = CGFloat(size) / 1024
    ctx.scaleBy(x: scale, y: scale)
    let tile = CGPath(roundedRect: CGRect(x: 100, y: 100, width: 824, height: 824),
                      cornerWidth: 186, cornerHeight: 186, transform: nil)
    ctx.saveGState()
    ctx.setShadow(offset: CGSize(width: 0, height: -18), blur: 28,
                  color: CGColor(gray: 0, alpha: 0.37))
    ctx.setFillColor(CGColor(red: 0.06, green: 0.18, blue: 0.19, alpha: 1))
    ctx.addPath(tile)
    ctx.fillPath()
    ctx.restoreGState()

    ctx.saveGState()
    ctx.addPath(tile)
    ctx.clip()
    let colors = [
        CGColor(red: 0.08, green: 0.35, blue: 0.31, alpha: 1),
        CGColor(red: 0.025, green: 0.12, blue: 0.16, alpha: 1),
    ] as CFArray
    let gradient = CGGradient(colorsSpace: space, colors: colors, locations: nil)!
    ctx.drawLinearGradient(gradient, start: CGPoint(x: 240, y: 900),
                           end: CGPoint(x: 780, y: 140), options: [.drawsBeforeStartLocation, .drawsAfterEndLocation])
    ctx.restoreGState()

    ctx.setLineWidth(45)
    ctx.setLineCap(.round)
    ctx.setLineJoin(.round)
    ctx.setStrokeColor(pale)
    let monitor = CGPath(roundedRect: CGRect(x: 250, y: 345, width: 524, height: 364),
                         cornerWidth: 53, cornerHeight: 53, transform: nil)
    ctx.addPath(monitor)
    ctx.strokePath()
    ctx.move(to: CGPoint(x: 512, y: 345))
    ctx.addLine(to: CGPoint(x: 512, y: 259))
    ctx.move(to: CGPoint(x: 412, y: 259))
    ctx.addLine(to: CGPoint(x: 612, y: 259))
    ctx.strokePath()

    // The incomplete ring and clock hand read clearly at Dock sizes.
    ctx.setStrokeColor(mint)
    ctx.setLineWidth(32)
    ctx.addArc(center: CGPoint(x: 512, y: 528), radius: 104,
               startAngle: .pi / 2, endAngle: .pi / 8, clockwise: false)
    ctx.strokePath()
    ctx.move(to: CGPoint(x: 512, y: 592))
    ctx.addLine(to: CGPoint(x: 512, y: 528))
    ctx.addLine(to: CGPoint(x: 558, y: 501))
    ctx.strokePath()
    return ctx.makeImage()!
}

for size in [16, 32, 128, 256, 512] {
    for density in [1, 2] {
        let suffix = density == 2 ? "@2x" : ""
        let url = destination.appendingPathComponent("icon_\(size)x\(size)\(suffix).png")
        let output = CGImageDestinationCreateWithURL(url as CFURL, UTType.png.identifier as CFString, 1, nil)!
        CGImageDestinationAddImage(output, icon(size: size * density), nil)
        precondition(CGImageDestinationFinalize(output), "Could not write icon image")
    }
}
