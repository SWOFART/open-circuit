import { pageDimensions, type DrawingPage } from "./drawingPage";

const STROKE_MM = 0.25;
const PRINT_FONT = "Open Circuit Export";
let printFont: Promise<FontFace> | undefined;

function xml(value: string): string {
  const escapes: Record<string, string> = {
    "&": "&amp;",
    "<": "&lt;",
    ">": "&gt;",
    '"': "&quot;",
    "'": "&apos;",
  };
  return value.replace(/[&<>"']/g, (character) => escapes[character]);
}

/** Write the PNG physical-pixel size (300 dpi) while keeping the canvas pixel data. */
async function withPrintDensity(blob: Blob): Promise<Blob> {
  const image = new Uint8Array(await blob.arrayBuffer());
  const source = new DataView(image.buffer);
  const chunk = new Uint8Array(21);
  const density = new DataView(chunk.buffer);
  density.setUint32(0, 9);
  chunk.set([112, 72, 89, 115], 4); // pHYs
  density.setUint32(8, 11811);
  density.setUint32(12, 11811);
  chunk[16] = 1; // pixels per metre
  let crc = 0xffffffff;
  for (const byte of chunk.subarray(4, 17)) {
    crc ^= byte;
    for (let bit = 0; bit < 8; bit++) {
      crc = (crc >>> 1) ^ ((crc & 1) === 1 ? 0xedb88320 : 0);
    }
  }
  density.setUint32(17, crc ^ 0xffffffff);
  const pieces: BlobPart[] = [image.slice(0, 8)];
  for (let offset = 8; offset + 12 <= image.length;) {
    const end = offset + 12 + source.getUint32(offset);
    if (end > image.length) throw new Error("Invalid PNG drawing.");
    const type = source.getUint32(offset + 4);
    if (type !== 0x70485973) pieces.push(image.slice(offset, end)); // pHYs
    if (type === 0x49484452) pieces.push(chunk); // IHDR
    offset = end;
  }
  return new Blob(pieces, { type: "image/png" });
}

export function drawingSvg(page: DrawingPage): string {
  const { width, height } = pageDimensions[page.size];
  const marks = page.marks.map((mark) => {
    switch (mark.kind) {
      case "polyline":
        return `<polyline points="${mark.points.map(({ x, y }) => `${x},${y}`).join(" ")}"/>`;
      case "circle":
        return `<circle cx="${mark.center.x}" cy="${mark.center.y}" r="${mark.radius}"${mark.filled ? ' fill="black"' : ""}/>`;
      case "text":
        return `<text x="${mark.position.x}" y="${mark.position.y}" font-size="${mark.size}" fill="black" stroke="none">${xml(mark.text)}</text>`;
    }
  });
  return [
    '<?xml version="1.0" encoding="UTF-8"?>',
    `<svg xmlns="http://www.w3.org/2000/svg" width="${width}mm" height="${height}mm" viewBox="0 0 ${width} ${height}">`,
    `<rect width="${width}" height="${height}" fill="white"/>`,
    `<g fill="none" stroke="black" stroke-width="${STROKE_MM}" font-family="sans-serif" text-anchor="middle" dominant-baseline="central">`,
    ...marks,
    "</g>",
    "</svg>",
    "",
  ].join("\n");
}

async function loadPrintFont(): Promise<FontFace> {
  const url = new URL("../../assets/fonts/DejaVuSans.ttf", import.meta.url);
  const face = new FontFace(PRINT_FONT, `url("${url.href}")`);
  await face.load();
  document.fonts.add(face);
  return face;
}

export async function drawingPng(page: DrawingPage): Promise<Blob> {
  await (printFont ??= loadPrintFont());
  const { width, height, pixels } = pageDimensions[page.size];
  const canvas = document.createElement("canvas");
  [canvas.width, canvas.height] = pixels;
  const context = canvas.getContext("2d");
  if (!context) throw new Error("Could not initialize PNG export.");
  context.scale(canvas.width / width, canvas.height / height);
  context.fillStyle = "white";
  context.fillRect(0, 0, width, height);
  context.fillStyle = "black";
  context.strokeStyle = "black";
  context.lineWidth = STROKE_MM;
  context.textAlign = "center";
  context.textBaseline = "middle";
  for (const mark of page.marks) {
    switch (mark.kind) {
      case "polyline": {
        const [first, ...rest] = mark.points;
        if (!first) break;
        context.beginPath();
        context.moveTo(first.x, first.y);
        for (const point of rest) context.lineTo(point.x, point.y);
        context.stroke();
        break;
      }
      case "circle":
        context.beginPath();
        context.arc(mark.center.x, mark.center.y, mark.radius, 0, 2 * Math.PI);
        if (mark.filled) context.fill();
        else context.stroke();
        break;
      case "text":
        context.font = `${mark.size}px "${PRINT_FONT}"`;
        context.fillText(mark.text, mark.position.x, mark.position.y);
        break;
    }
  }
  const encoded = await new Promise<Blob>((resolve, reject) => {
    canvas.toBlob((blob) => {
      if (blob) resolve(blob);
      else reject(new Error("Could not encode the PNG drawing."));
    }, "image/png");
  });
  return withPrintDensity(encoded);
}
