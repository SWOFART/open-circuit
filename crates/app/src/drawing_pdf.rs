//! Minimal deterministic vector PDF encoding for the application's drawing page.

use std::collections::BTreeSet;
use std::fmt::Write;

use serde_json::Value;

const FONT: &[u8] = include_bytes!("../../../assets/fonts/DejaVuSans.ttf");
const POINTS_PER_MM: f64 = 72.0 / 25.4;
const FONT_ID: usize = 3;

fn word(bytes: &[u8], offset: usize) -> u16 {
    u16::from_be_bytes(bytes[offset..offset + 2].try_into().unwrap())
}

fn long(bytes: &[u8], offset: usize) -> usize {
    u32::from_be_bytes(bytes[offset..offset + 4].try_into().unwrap()) as usize
}

fn table(name: &[u8; 4]) -> &'static [u8] {
    // The font is an immutable, checked-in resource.
    let record = FONT[12..12 + usize::from(word(FONT, 4)) * 16]
        .as_chunks::<16>()
        .0
        .iter()
        .find(|record| &record[..4] == name)
        .expect("DejaVu Sans has the required TrueType table");
    let offset = long(record, 8);
    &FONT[offset..offset + long(record, 12)]
}

struct FontMetrics {
    cmap: &'static [u8],
    hmtx: &'static [u8],
    last_metric: usize,
    units_per_em: f64,
}

impl FontMetrics {
    fn new() -> Self {
        let cmap = table(b"cmap");
        let cmap = cmap[4..4 + usize::from(word(cmap, 2)) * 8]
            .as_chunks::<8>()
            .0
            .iter()
            .map(|record| &cmap[long(record, 4)..])
            .find(|subtable| word(subtable, 0) == 4)
            .expect("DejaVu Sans has a Unicode BMP cmap");
        Self {
            cmap,
            hmtx: table(b"hmtx"),
            last_metric: usize::from(word(table(b"hhea"), 34)) - 1,
            units_per_em: f64::from(word(table(b"head"), 18)),
        }
    }

    fn glyph(&self, code: u16) -> u16 {
        let count = usize::from(word(self.cmap, 6)) / 2;
        for index in 0..count {
            let end = word(self.cmap, 14 + 2 * index);
            if code > end {
                continue;
            }
            let start = word(self.cmap, 16 + 2 * (count + index));
            if code < start {
                return 0;
            }
            let delta = word(self.cmap, 16 + 2 * (2 * count + index));
            let offset_position = 16 + 2 * (3 * count + index);
            let offset = usize::from(word(self.cmap, offset_position));
            if offset == 0 {
                return code.wrapping_add(delta);
            }
            let glyph = word(
                self.cmap,
                offset_position + offset + 2 * usize::from(code - start),
            );
            return if glyph == 0 {
                0
            } else {
                glyph.wrapping_add(delta)
            };
        }
        0
    }

    fn width(&self, glyph: u16) -> f64 {
        f64::from(word(
            self.hmtx,
            usize::from(glyph).min(self.last_metric) * 4,
        )) / self.units_per_em
    }
}

fn number(value: &Value) -> Result<f64, String> {
    value
        .as_f64()
        .filter(|n| n.abs() <= 10_000.0)
        .ok_or_else(|| "Invalid PDF page coordinate or size.".into())
}

fn point(value: &Value, page_height: f64) -> Result<(f64, f64), String> {
    Ok((number(&value["x"])?, page_height - number(&value["y"])?))
}

fn page_content(
    marks: &[Value],
    height: f64,
    metrics: &FontMetrics,
    characters: &mut BTreeSet<u16>,
) -> Result<String, String> {
    let mut output = format!("{POINTS_PER_MM:.8} 0 0 {POINTS_PER_MM:.8} 0 0 cm\n0 G 0 g 0.25 w\n");
    for mark in marks {
        match mark["kind"].as_str() {
            Some("polyline") => {
                let points = mark["points"]
                    .as_array()
                    .filter(|points| points.len() >= 2)
                    .ok_or("Invalid PDF polyline.")?;
                for (index, value) in points.iter().enumerate() {
                    let (x, y) = point(value, height)?;
                    let operator = if index == 0 { 'm' } else { 'l' };
                    writeln!(output, "{x:.4} {y:.4} {operator}").unwrap();
                }
                output.push_str("S\n");
            }
            Some("circle") => {
                let (x, y) = point(&mark["center"], height)?;
                let radius = number(&mark["radius"])?;
                if radius <= 0.0 {
                    return Err("Invalid PDF circle radius.".into());
                }
                let control = radius * 0.552_284_749_8;
                let (l, r, b, t) = (x - radius, x + radius, y - radius, y + radius);
                let (xl, xr, yb, yt) = (x - control, x + control, y - control, y + control);
                write!(output, "{r:.4} {y:.4} m\n{r:.4} {yt:.4} {xr:.4} {t:.4} {x:.4} {t:.4} c\n{xl:.4} {t:.4} {l:.4} {yt:.4} {l:.4} {y:.4} c\n{l:.4} {yb:.4} {xl:.4} {b:.4} {x:.4} {b:.4} c\n{xr:.4} {b:.4} {r:.4} {yb:.4} {r:.4} {y:.4} c\n").unwrap();
                output.push_str(
                    if mark["filled"].as_bool().ok_or("Invalid PDF circle fill.")? {
                        "f\n"
                    } else {
                        "S\n"
                    },
                );
            }
            Some("text") => {
                let (x, y) = point(&mark["position"], height)?;
                let size = number(&mark["size"])?;
                let text = mark["text"]
                    .as_str()
                    .filter(|text| text.len() <= 65_536)
                    .ok_or("Invalid PDF text.")?;
                if !(0.1..=100.0).contains(&size) {
                    return Err("Invalid PDF font size.".into());
                }
                let mut encoded = String::new();
                let mut width = 0.0;
                for character in text.chars() {
                    let code = u16::try_from(u32::from(character)).unwrap_or(0xfffd);
                    characters.insert(code);
                    width += size * metrics.width(metrics.glyph(code));
                    write!(encoded, "{code:04X}").unwrap();
                }
                let left = x - width / 2.0;
                let baseline = y - size * 0.35;
                writeln!(
                    output,
                    "BT /F1 {size:.4} Tf {left:.4} {baseline:.4} Td <{encoded}> Tj ET"
                )
                .unwrap();
            }
            _ => return Err("Unknown PDF drawing mark.".into()),
        }
    }
    Ok(output)
}

fn stream(bytes: &[u8], fields: &str) -> Vec<u8> {
    let mut object = format!("<< /Length {} {fields} >>\nstream\n", bytes.len()).into_bytes();
    object.extend_from_slice(bytes);
    object.extend_from_slice(b"\nendstream");
    object
}

fn finish(objects: Vec<Vec<u8>>) -> Vec<u8> {
    let mut document = b"%PDF-1.4\n%\xE2\xE3\xCF\xD3\n".to_vec();
    let mut offsets = vec![0];
    for (index, object) in objects.iter().enumerate() {
        offsets.push(document.len());
        document.extend(format!("{} 0 obj\n", index + 1).bytes());
        document.extend(object);
        document.extend_from_slice(b"\nendobj\n");
    }
    let xref = document.len();
    document.extend(format!("xref\n0 {}\n0000000000 65535 f \n", offsets.len()).bytes());
    for offset in &offsets[1..] {
        document.extend(format!("{offset:010} 00000 n \n").bytes());
    }
    document.extend(
        format!(
            "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n",
            offsets.len()
        )
        .bytes(),
    );
    document
}

pub(crate) fn encode(pages: &Value) -> Result<Vec<u8>, String> {
    let pages = pages
        .as_array()
        .filter(|pages| !pages.is_empty())
        .ok_or("Invalid PDF page count.")?;
    let metrics = FontMetrics::new();
    let mut characters = BTreeSet::new();
    let mut kids = String::new();
    let mut objects = vec![Vec::new(); 8];
    for page in pages {
        let (width, height) = match page["size"].as_str() {
            Some("a4") => (297.0, 210.0),
            Some("a3") => (420.0, 297.0),
            _ => return Err("Invalid PDF page size.".into()),
        };
        let marks = page["marks"].as_array().ok_or("Invalid PDF marks.")?;
        let content = page_content(marks, height, &metrics, &mut characters)?;
        let page_id = objects.len() + 1;
        write!(kids, "{page_id} 0 R ").unwrap();
        objects.push(format!(
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {:.4} {:.4}] /Resources << /Font << /F1 {FONT_ID} 0 R >> >> /Contents {} 0 R >>",
            width * POINTS_PER_MM, height * POINTS_PER_MM, page_id + 1
        ).into_bytes());
        objects.push(stream(content.as_bytes(), ""));
    }
    let mut glyph_map = vec![0; 2 * (usize::from(characters.last().copied().unwrap_or(0)) + 1)];
    let mut widths = String::new();
    let mut unicode = String::from(
        "/CIDInit /ProcSet findresource begin\n12 dict begin\nbegincmap\n/CIDSystemInfo << /Registry (Adobe) /Ordering (UCS) /Supplement 0 >> def\n/CMapName /OpenCircuitUnicode def\n/CMapType 2 def\n1 begincodespacerange\n<0000> <FFFF>\nendcodespacerange\n",
    );
    let characters: Vec<_> = characters.into_iter().collect();
    for &code in &characters {
        let glyph = metrics.glyph(code);
        let offset = usize::from(code) * 2;
        glyph_map[offset..offset + 2].copy_from_slice(&glyph.to_be_bytes());
        write!(widths, "{code} [{:.3}] ", metrics.width(glyph) * 1000.0).unwrap();
    }
    for batch in characters.chunks(100) {
        writeln!(unicode, "{} beginbfchar", batch.len()).unwrap();
        for code in batch {
            writeln!(unicode, "<{code:04X}> <{code:04X}>").unwrap();
        }
        unicode.push_str("endbfchar\n");
    }
    unicode.push_str("endcmap\nCMapName currentdict /CMap defineresource pop\nend\nend\n");
    objects[0] = b"<< /Type /Catalog /Pages 2 0 R >>".to_vec();
    objects[1] = format!("<< /Type /Pages /Count {} /Kids [{kids}] >>", pages.len()).into_bytes();
    objects[2] = b"<< /Type /Font /Subtype /Type0 /BaseFont /DejaVuSans /Encoding /Identity-H /DescendantFonts [4 0 R] /ToUnicode 8 0 R >>".to_vec();
    objects[3] = format!("<< /Type /Font /Subtype /CIDFontType2 /BaseFont /DejaVuSans /CIDSystemInfo << /Registry (Adobe) /Ordering (Identity) /Supplement 0 >> /FontDescriptor 5 0 R /CIDToGIDMap 7 0 R /W [{widths}] >>").into_bytes();
    objects[4] = b"<< /Type /FontDescriptor /FontName /DejaVuSans /Flags 32 /FontBBox [-1021 -463 1794 1233] /ItalicAngle 0 /Ascent 928 /Descent -236 /CapHeight 729 /StemV 80 /FontFile2 6 0 R >>".to_vec();
    objects[5] = stream(FONT, &format!("/Length1 {}", FONT.len()));
    objects[6] = stream(&glyph_map, "");
    objects[7] = stream(unicode.as_bytes(), "");
    Ok(finish(objects))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn vector_pages_embed_the_font_and_map_german_and_cyrillic_text() {
        let pages = json!([
            {"size":"a4","marks":[
                {"kind":"text","position":{"x":30,"y":25},"text":"Über Щит","size":4}
            ]},
            {"size":"a3","marks":[
                {"kind":"polyline","points":[{"x":0,"y":0},{"x":1,"y":1}]},
                {"kind":"circle","center":{"x":2,"y":3},"radius":1,"filled":true}
            ]}
        ]);
        let encoded = encode(&pages).unwrap();
        assert_eq!(encoded, encode(&pages).unwrap());
        let text = String::from_utf8_lossy(&encoded);
        for fragment in [
            "/Count 2",
            "/FontFile2",
            "/MediaBox",
            "<00DC> <00DC>",
            "<0429> <0429>",
        ] {
            assert!(text.contains(fragment), "{fragment}");
        }
        let metrics = FontMetrics::new();
        assert_ne!(metrics.glyph('Ü' as u16), 0);
        assert_ne!(metrics.glyph('Щ' as u16), 0);
        assert!(encode(&json!([{"size":"a4","marks":[{"kind":"text"}]}])).is_err());
    }

    #[test]
    fn a_long_bill_of_materials_keeps_all_pages() {
        let pages = Value::Array(vec![json!({"size": "a4", "marks": []}); 130]);
        let pdf = encode(&pages).unwrap();
        assert!(String::from_utf8_lossy(&pdf).contains("/Count 130"));
    }
}
